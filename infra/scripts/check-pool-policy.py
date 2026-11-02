#!/usr/bin/env python3
"""Pure policy and host-fence regressions; no AWS calls or Docker required."""
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import Mock, patch
from botocore.exceptions import ClientError

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "runtime"))
sys.path.insert(0, str(ROOT / "pool"))
from policy import compile_policy, retirement_plan, target_instances
from planning import project

DOCUMENT = json.loads((ROOT / "policy/pool.json").read_text())
POLICY = compile_policy(DOCUMENT, "lookahead")


class PolicyTests(unittest.TestCase):
    def test_declared_profiles_and_invalid_bounds(self):
        for name in DOCUMENT["profiles"]:
            self.assertEqual(compile_policy(DOCUMENT, name)["profile"], name)
        for key, value in [("max_instances", 6), ("executions_per_instance", 2), ("max_instances", True)]:
            document = copy.deepcopy(DOCUMENT)
            document["limits"][key] = value
            with self.assertRaises(ValueError):
                compile_policy(document, "lookahead")
        with self.assertRaises(ValueError):
            compile_policy(DOCUMENT, "unreviewed-override")

    def test_placement_and_renewal_are_explicit_safety_policy(self):
        self.assertEqual(POLICY["placement"], {"availability_zone_count": 1,
            "replacement_min_healthy_percentage": 0, "replacement_max_healthy_percentage": 100})
        for section, field, value in [("placement", "availability_zone_count", 2),
                ("placement", "replacement_max_healthy_percentage", 110),
                ("queue", "renewal_seconds", DOCUMENT["queue"]["visibility_seconds"])]:
            document = copy.deepcopy(DOCUMENT)
            document[section][field] = value
            with self.assertRaises(ValueError):
                compile_policy(document, "lookahead")

    def target(self, ready=0, forecast=0, pending=0, now=1000, activity=0, sleeping=True, policy=POLICY):
        return target_instances(policy, ready, forecast, pending, now, activity, sleeping)

    def test_prediction_is_bounded_and_separate_from_reactive(self):
        self.assertEqual(self.target(ready=1, forecast=100), 5)
        prewarmed = compile_policy(DOCUMENT, "prewarmed")
        self.assertEqual(self.target(sleeping=False, policy=prewarmed), 5)
        self.assertEqual(self.target(policy=prewarmed), 0)
        reactive = compile_policy(DOCUMENT, "reactive")
        self.assertEqual(self.target(ready=1, forecast=100, policy=reactive), 1)
        self.assertEqual(self.target(pending=1), 1)
        self.assertEqual(self.target(), 0)
        self.assertEqual(self.target(activity=999), 1)
        self.assertEqual(self.target(sleeping=False), 1)

    def test_gate_tree_future_demand_failure_and_horizon(self):
        jobs = [("gate", {"wall_seconds": 60})] + [(str(n), {"wall_seconds": 10}) for n in range(8)]
        edges = [("gate", str(n)) for n in range(8)]
        self.assertEqual(project(jobs, edges, horizon=120)["potential_slots"], 8)
        self.assertEqual(project(jobs, edges, horizon=30)["potential_slots"], 1)
        self.assertEqual(project(jobs, edges, {"gate": "failed"}, 120)["potential_slots"], 0)
        self.assertEqual(project(jobs, edges, {"gate": "completed"}, 120)["potential_slots"], 8)

    def test_deep_tree_and_sequential_boundaries(self):
        jobs = [(str(n), {"wall_seconds": 1}) for n in range(1500)]
        edges = [(str(n), str(n+1)) for n in range(1499)]
        self.assertEqual(project(jobs, edges, horizon=2000)["potential_slots"], 1)
        with self.assertRaises(ValueError):
            project(jobs, edges + [("1499", "0")], horizon=2000)

    def test_drain_requires_matching_ack_and_actual_unprotection(self):
        instances = {"vm": {"ProtectedFromScaleIn": True}}
        hosts = {"vm": {"state": "ready", "active": 1}}
        safe, selected, transition = retirement_plan(0, 1, instances, hosts, {}, {})
        self.assertEqual((safe, selected, transition), (1, ["vm"], "await_drain"))
        hosts["vm"].update(state="drained", active=0, polling=False, drain_id="old")
        retiring = {"vm": "current"}
        for protected in [True, False]:
            instances["vm"]["ProtectedFromScaleIn"] = protected
            self.assertEqual(retirement_plan(0, 1, instances, hosts, retiring, retiring)[0], 1)
        hosts["vm"]["drain_id"] = "current"
        self.assertEqual(retirement_plan(0, 1, instances, hosts, retiring, retiring)[0], 0)

    def test_scale_in_without_known_eligible_hosts_holds_current(self):
        instances = {"vm": {"ProtectedFromScaleIn": True}}
        for hosts in [{}, {"vm": {"state": "quarantined", "active": 0, "polling": False}},
                      {"vm": {"state": "draining", "active": 0, "polling": True}}]:
            self.assertEqual(retirement_plan(0, 1, instances, hosts, {}, {}),
                             (1, [], "await_unknown_capacity"))
        # Desired capacity awaiting launch has no host that could acknowledge.
        self.assertEqual(retirement_plan(0, 3, {}, {}, {}, {}),
                         (3, [], "await_unknown_capacity"))

    def test_controller_requires_independent_poll_and_execution_acknowledgement(self):
        instances = {"vm": {"ProtectedFromScaleIn": False}}
        hosts = {"vm": {"state": "drained", "active": 0, "polling": False, "drain_id": "token"}}
        retiring = {"vm": "token"}
        self.assertEqual(retirement_plan(0, 1, instances, hosts, retiring, retiring)[0], 0)
        for active, polling in [(1, False), (0, True), (1, True), (0, None)]:
            hosts["vm"].update(active=active, polling=polling)
            self.assertEqual(retirement_plan(0, 1, instances, hosts, retiring, retiring)[0], 1)
        # A heartbeat from an older protocol lacking the poll fence is insufficient.
        hosts["vm"].update(active=0)
        hosts["vm"].pop("polling")
        self.assertEqual(retirement_plan(0, 1, instances, hosts, retiring, retiring)[0], 1)

    def test_rebound_completes_retirement_without_resuming(self):
        instances = {"vm": {"ProtectedFromScaleIn": False}}
        hosts = {"vm": {"state": "drained", "active": 0, "polling": False, "drain_id": "token"}}
        retiring = {"vm": "token"}
        self.assertEqual(retirement_plan(5, 1, instances, hosts, retiring, retiring)[0], 0)
        self.assertEqual(retirement_plan(5, 0, {}, {}, {}, {})[0], 5)
        # Missing/stale host acknowledgements fail closed.
        self.assertEqual(retirement_plan(0, 1, instances, {}, retiring, retiring)[0], 1)


class HostFenceTests(unittest.TestCase):
    def load_agent(self):
        spec = importlib.util.spec_from_file_location("test_host_agent", ROOT / "pool/agent.py")
        module = importlib.util.module_from_spec(spec)
        with patch.dict(os.environ, {"BUCKET": "test", "NODE_QUEUE": "test", "CONTROL_QUEUE": "test", "POOL_POLICY": json.dumps(POLICY)}), patch("boto3.client", return_value=Mock()):
            spec.loader.exec_module(module)
        module.s3 = Mock()
        module.sqs = Mock()
        module.scaling = Mock()
        module.instance_id = "test-vm"
        module.asg = "test-asg"
        module.runtime_version = "test"
        module.startup_cleanup = {}
        module.healthy = True
        return module

    def drain_commands(self, agent):
        agent.s3.get_object.side_effect = lambda **kwargs: {
            "Body": io.BytesIO(b'{"action":"drain","drain_id":"test-token"}')}

    def heartbeat(self, agent):
        return json.loads(agent.s3.put_object.call_args.kwargs["Body"])

    def test_drain_persists_before_unprotect_and_blocks_new_polls(self):
        agent = self.load_agent()
        self.drain_commands(agent)
        with tempfile.TemporaryDirectory() as directory:
            agent.RETIREMENT = Path(directory) / "retirement.json"
            def assert_fence(**kwargs):
                self.assertTrue(agent.draining)
                self.assertFalse(agent.polling)
                self.assertEqual(json.loads(agent.RETIREMENT.read_text())["drain_id"], "test-token")
            agent.scaling.set_instance_protection.side_effect = assert_fence
            agent.publish_heartbeat()
            self.assertFalse(agent.poll_once())
            agent.sqs.receive_message.assert_not_called()
            agent.sqs.change_message_visibility.assert_not_called()
            body = self.heartbeat(agent)
            self.assertEqual((body["state"], body["drain_id"], body["executions_per_instance"]), ("drained", "test-token", 1))

    def test_pending_empty_poll_fences_drain_until_response(self):
        agent = self.load_agent()
        self.drain_commands(agent)
        self.assertTrue(agent.reserve_poll())
        started = agent.poll_started_at
        with tempfile.TemporaryDirectory() as directory:
            agent.RETIREMENT = Path(directory) / "retirement.json"
            agent.publish_heartbeat()
            body = self.heartbeat(agent)
            self.assertEqual(body["state"], "draining")
            self.assertTrue(body["polling"])
            self.assertEqual(body["poll_started_at"], started)
            self.assertEqual(body["active"], 0)
            agent.scaling.set_instance_protection.assert_not_called()
            agent.finish_poll(False)
            agent.publish_heartbeat()
            self.assertEqual(self.heartbeat(agent)["state"], "drained")
            agent.scaling.set_instance_protection.assert_called_once()

    def test_reserved_message_executes_and_cleans_up_before_unprotection(self):
        agent = self.load_agent()
        self.drain_commands(agent)
        received = threading.Event()
        release = threading.Event()
        errors = []
        message = {"receipt": {"contest": "contest", "submission": 1, "run": "run"},
                   "node": "node", "task": {"phase": "run", "inputs": {}, "outputs": []}, "artifacts": {}}
        record = {"Body": json.dumps(message), "ReceiptHandle": "reserved", "Attributes": {"ApproximateReceiveCount": "1"}}
        def receive(**kwargs):
            received.set()
            if not release.wait(5):
                raise RuntimeError("test never released reserved receive")
            return {"Messages": [record]}
        agent.sqs.receive_message.side_effect = receive
        agent.s3.head_object.side_effect = ClientError({"Error": {"Code": "404"}}, "HeadObject")
        def run_once():
            try:
                agent.poll_once()
            except Exception as exc:
                errors.append(exc)
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"IMAGE": "test"}):
            agent.RETIREMENT = Path(directory) / "retirement.json"
            agent.JOB_ROOT = Path(directory) / "jobs"
            agent.JOB_ROOT.mkdir()
            process = Mock(returncode=0)
            def execute(**kwargs):
                self.assertTrue(agent.draining)
                self.assertEqual(agent.active, 1)
                self.assertFalse(agent.polling)
                self.assertFalse(agent.unprotected)
                agent.scaling.set_instance_protection.assert_not_called()
                work = next(agent.JOB_ROOT.iterdir())
                (work / "result.json").write_text('{"exit_code":0,"artifacts":{}}')
                return b"", b""
            process.communicate.side_effect = execute
            def inspect(command, **kwargs):
                if command[1] == "inspect":
                    return b'{"Runtime":"runsc","NetworkMode":"none","ReadonlyRootfs":true}'
                return b""
            def cleanup(command, **kwargs):
                self.assertEqual(agent.active, 1)
                agent.publish_heartbeat()
                self.assertEqual(self.heartbeat(agent)["state"], "draining")
                agent.scaling.set_instance_protection.assert_not_called()
                return Mock(returncode=0)
            with patch.object(agent.subprocess, "Popen", return_value=process), patch.object(agent.subprocess, "check_output", side_effect=inspect), patch.object(agent.subprocess, "run", side_effect=cleanup):
                thread = threading.Thread(target=run_once)
                thread.start()
                self.assertTrue(received.wait(5))
                agent.publish_heartbeat()
                self.assertTrue(self.heartbeat(agent)["polling"])
                agent.scaling.set_instance_protection.assert_not_called()
                release.set()
                thread.join(5)
                self.assertFalse(thread.is_alive())
            self.assertFalse(errors)
            self.assertEqual(agent.active, 0)
            self.assertFalse(agent.polling)
            self.assertFalse(agent.unprotected)
            self.assertEqual(list(agent.JOB_ROOT.iterdir()), [])
            agent.sqs.change_message_visibility.assert_not_called()
            agent.sqs.delete_message.assert_called_once()
            writes = [call.kwargs for call in agent.s3.put_object.call_args_list]
            attempts = [json.loads(call["Body"]) for call in writes if call["Key"].startswith("workers/attempts/")]
            results = [json.loads(call["Body"]) for call in writes if call["Key"].startswith("results/")]
            self.assertEqual(len(attempts), 1)
            self.assertEqual(len(results), 1)
            self.assertEqual(attempts[0]["receive_count"], 1)
            self.assertEqual(attempts[0]["admission"]["mode"], "reserved_before_drain")
            self.assertEqual(results[0]["admission"], attempts[0]["admission"])
            agent.publish_heartbeat()
            self.assertEqual(self.heartbeat(agent)["state"], "drained")
            agent.scaling.set_instance_protection.assert_called_once()
            self.assertFalse(agent.poll_once())
            self.assertEqual(agent.sqs.receive_message.call_count, 1)

    def test_receive_error_releases_reservation_without_immediate_requeue(self):
        agent = self.load_agent()
        self.drain_commands(agent)
        with tempfile.TemporaryDirectory() as directory:
            agent.RETIREMENT = Path(directory) / "retirement.json"
            def failed_receive(**kwargs):
                agent.publish_heartbeat()
                agent.scaling.set_instance_protection.assert_not_called()
                raise TimeoutError("unobserved SQS response")
            agent.sqs.receive_message.side_effect = failed_receive
            with self.assertRaises(TimeoutError):
                agent.poll_once()
            self.assertFalse(agent.polling)
            self.assertEqual(agent.active, 0)
            agent.sqs.change_message_visibility.assert_not_called()
            agent.publish_heartbeat()
            self.assertEqual(self.heartbeat(agent)["state"], "drained")

    def test_single_reservation_transfers_without_an_idle_gap(self):
        agent = self.load_agent()
        self.assertTrue(agent.reserve_poll())
        with self.assertRaises(RuntimeError):
            agent.reserve_poll()
        admission = agent.finish_poll(True)
        self.assertEqual(admission["mode"], "poll_reserved")
        self.assertEqual(agent.active, 1)
        self.assertFalse(agent.polling)
        with self.assertRaises(RuntimeError):
            agent.reserve_poll()

    def test_active_execution_does_not_acknowledge_drain(self):
        agent = self.load_agent()
        agent.active = 1
        self.drain_commands(agent)
        with tempfile.TemporaryDirectory() as directory:
            agent.RETIREMENT = Path(directory) / "retirement.json"
            agent.publish_heartbeat()
        agent.scaling.set_instance_protection.assert_not_called()
        body = self.heartbeat(agent)
        self.assertEqual(body["state"], "draining")
        self.assertEqual(body["active"], 1)


if __name__ == "__main__":
    unittest.main()
