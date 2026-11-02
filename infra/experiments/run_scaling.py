#!/usr/bin/env python3
"""Real AWS observations. Never changes desired capacity or scaling policy directly."""
import argparse
import collections
import datetime
import hashlib
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request
import uuid

import boto3
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest
from botocore.exceptions import ClientError

from workloads import body, forecast_tree, pruned_tree

REGION = "ap-northeast-1"


class Experiment:
    def __init__(self, stack, report_path, resume=False):
        self.stack = stack
        self.session = boto3.Session(region_name=REGION)
        self.cfn = self.session.client("cloudformation")
        self.outputs = {v["OutputKey"]: v["OutputValue"] for v in self.cfn.describe_stacks(StackName=stack)["Stacks"][0]["Outputs"]}
        self.s3, self.sqs = self.session.client("s3"), self.session.client("sqs")
        self.scaling, self.functions = self.session.client("autoscaling"), self.session.client("lambda")
        self.report_path = report_path
        self.contest = "scaling-" + uuid.uuid4().hex[:12]
        self.report = {"started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "policy_profile": self.outputs["PolicyProfile"], "policy_sha256": self.outputs["PolicyFingerprint"],
            "checks": {}, "scenarios": {}, "samples": []}
        self.last_sample = 0
        self.worker_seen = {}
        self.states = []
        self.resume_forecast = resume
        if resume:
            self.report = json.loads(report_path.read_text())
            self.report.pop("failure_type", None)
            for obj in self.objects("checkpoints/"):
                state = self.read(obj["Key"])
                if state["receipt"]["request_id"] == "forecast-tree":
                    self.forecast_receipt = state["receipt"]
                    self.contest = state["receipt"]["contest"]
                    break
            else:
                raise ValueError("no forecast checkpoint available to resume")
            for sample in self.report["samples"]:
                for host in sample["hosts"]:
                    self.worker_seen.setdefault(host["worker"], host["first_observed_at"])

    def save(self):
        self.report_path.parent.mkdir(parents=True, exist_ok=True)
        self.report_path.write_text(json.dumps(self.report, indent=2) + "\n")

    def objects(self, prefix, delimiter=None):
        args = {"Bucket": self.outputs["Bucket"], "Prefix": prefix}
        if delimiter:
            args["Delimiter"] = delimiter
        for page in self.s3.get_paginator("list_objects_v2").paginate(**args):
            for obj in page.get("Contents", []):
                yield obj

    def read(self, key):
        return json.loads(self.s3.get_object(Bucket=self.outputs["Bucket"], Key=key)["Body"].read())

    def api(self, method, data=None, query=None):
        url = self.outputs["ApiUrl"] + ("?" + urllib.parse.urlencode(query) if query else "")
        encoded = json.dumps(data).encode() if data is not None else b""
        req = AWSRequest(method=method, url=url, data=encoded, headers={"Content-Type": "application/json",
            "X-Amz-Content-Sha256": hashlib.sha256(encoded).hexdigest()})
        SigV4Auth(self.session.get_credentials().get_frozen_credentials(), "lambda", REGION).add_auth(req)
        with urllib.request.urlopen(urllib.request.Request(url, data=encoded if method == "POST" else None,
                headers=dict(req.headers.items()), method=method), timeout=35) as response:
            return json.load(response)

    def capacity(self, action):
        response = self.functions.invoke(FunctionName=self.outputs["CapacityFunction"], Payload=json.dumps({"action": action}).encode())
        data = json.loads(response["Payload"].read())
        return response.get("FunctionError"), data

    def sample(self, force=False):
        if not force and self.report["samples"] and time.monotonic() - self.last_sample < 5:
            return self.report["samples"][-1]
        group = self.scaling.describe_auto_scaling_groups(AutoScalingGroupNames=[self.outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
        members = {v["InstanceId"]: v for v in group["Instances"]}
        assert group["MaxSize"] == 5
        hosts = {}
        for obj in self.objects("workers/", delimiter="/"):
            host = self.read(obj["Key"])
            if host.get("instance_id") not in members or time.time() - host.get("heartbeat_at", 0) > 20:
                continue
            previous = hosts.get(host["instance_id"])
            if previous is None or previous["heartbeat_at"] < host["heartbeat_at"]:
                hosts[host["instance_id"]] = host
        observations = []
        for instance, host in hosts.items():
            assert host["executions_per_instance"] == 1 and host["active"] in [0, 1]
            protected = members[instance]["ProtectedFromScaleIn"]
            self.worker_seen.setdefault(host["worker"], time.time())
            observations.append({"worker": host["worker"], "state": host["state"], "active": host["active"],
                                 "protected": protected, "first_observed_at": self.worker_seen[host["worker"]]})
        queues = {}
        for name in ["NodeQueue", "ControlQueue"]:
            v = self.sqs.get_queue_attributes(QueueUrl=self.outputs[name], AttributeNames=[
                "ApproximateNumberOfMessages", "ApproximateNumberOfMessagesNotVisible"])["Attributes"]
            queues[name] = {"visible": int(v["ApproximateNumberOfMessages"]), "inflight": int(v["ApproximateNumberOfMessagesNotVisible"])}
        snapshot = {"at": time.time(), "desired": group["DesiredCapacity"], "physical_members": len(members),
                    "hosts": observations, "queue": queues}
        self.last_sample = time.monotonic()
        self.report["samples"].append(snapshot)
        self.save()
        return snapshot

    def wait(self, predicate, timeout, label):
        start = time.monotonic()
        while time.monotonic() - start < timeout:
            self.sample()
            value = predicate()
            if value:
                print(label, flush=True)
                return value
            time.sleep(3)
        raise TimeoutError(label)

    def submit(self, data):
        receipt = self.api("POST", data)
        print("Accepted " + data["request_id"], flush=True)
        self.report["scenarios"].setdefault(data["request_id"], {})["receipt"] = receipt
        self.save()
        return receipt

    def view(self, receipt):
        return self.api("GET", query={"contest": receipt["contest"], "request_id": receipt["request_id"]})

    def complete(self, receipt, timeout=600):
        def done():
            view = self.view(receipt)
            self.report["scenarios"].setdefault(receipt["request_id"], {})["last_view"] = view
            return view if view["status"] in ["completed", "infrastructure_error"] else None
        view = self.wait(done, timeout, "Terminal " + receipt["request_id"])
        self.report["scenarios"][receipt["request_id"]].setdefault("accept_to_observed_terminal_seconds", round(
            time.time() - datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp(), 3))
        state = self.read(f"checkpoints/{receipt['contest']}/{receipt['submission']}/{receipt['run']}.json")
        self.states.append(state)
        self.save()
        return state

    def cleanup(self, state):
        expected = {node for node, result in state["results"].items() if not result.get("skipped")}
        prefix = "workers/cleanups/" + state["receipt"]["run"] + "/"
        def all_clean():
            objects = list(self.objects(prefix))
            values = [self.read(v["Key"]) for v in objects]
            covered = {v["Key"][len(prefix):].split('/')[0] for v in objects}
            return values if expected <= covered and all(v["container_absent"] and v["workdir_absent"] for v in values) else None
        return self.wait(all_clean, 90, "Cleanup verified")

    def only_one_ready(self):
        s = self.sample(force=True)
        return s if s["desired"] == 1 and s["physical_members"] == 1 and len(s["hosts"]) == 1 and s["hosts"][0]["state"] == "ready" else None

    def inspect_declarations(self):
        policies = self.scaling.describe_policies(AutoScalingGroupName=self.outputs["AutoScalingGroup"])["ScalingPolicies"]
        assert len(policies) == 2 and all(p["PolicyType"] == "StepScaling" and p["AdjustmentType"] == "ExactCapacity" for p in policies)
        template = self.cfn.get_template(StackName=self.stack)["TemplateBody"]
        if isinstance(template, str):
            template = json.loads(template)
        group = next(v["Properties"] for v in template["Resources"].values() if v["Type"] == "AWS::AutoScaling::AutoScalingGroup")
        assert str(group["MaxSize"]) == "5" and group["NewInstancesProtectedFromScaleIn"]
        assert group["InstanceMaintenancePolicy"]["MaxHealthyPercentage"] == 100
        assert len(group["VPCZoneIdentifier"]) == 1
        assert "autoscaling:SetDesiredCapacity" not in json.dumps(template)
        self.report["checks"]["cloudformation_step_scaling_policies_present"] = True

    def run(self):
        self.inspect_declarations()
        self.wait(lambda: any(h["state"] == "ready" and h["protected"] for h in self.sample(force=True)["hosts"]),
                  300, "At least one protected worker ready")
        if self.resume_forecast:
            receipt = self.forecast_receipt
        else:
            receipt = self.submit(forecast_tree(self.contest, "forecast-tree"))
            self.wait(lambda: any(h["active"] for h in self.sample(force=True)["hosts"]), 90, "Native work active")
            error, _ = self.capacity("sleep")
            assert error, "sleep accepted known active work"
            self.report["checks"]["sleep_rejected_while_work_active"] = True
        tree = self.complete(receipt)
        assert tree["view"]["verdict"] == "AC" and tree["view"]["completed_cases"] == 13
        first_leaf = min(tree["results"][node]["execution"]["started_ns"] / 1e9
            for node, task in tree["jobs"] if task["phase"] == "run" and task["case"] >= 8)
        ready_five = next(s["at"] for s in self.report["samples"] if len([h for h in s["hosts"] if h["state"] == "ready"]) == 5)
        self.report["scenarios"]["forecast-tree"].update(five_ready_before_wide_layer_seconds=round(first_leaf - ready_five, 3))
        self.report["checks"]["five_VMs_observed_ready_before_wide_tree_layer"] = ready_five < first_leaf
        print(f"Observed five-ready lead over wide layer: {first_leaf-ready_five:.3f}s", flush=True)
        await_cleanup = self.cleanup(tree)
        assert len(await_cleanup) == 28
        pruned = self.complete(self.submit(pruned_tree(self.contest, "pruned-tree")))
        assert pruned["view"]["verdict"] == "WA" and pruned["view"]["completed_cases"] == 3 and pruned["view"]["skipped_cases"] == 3
        for node, task in pruned["jobs"]:
            if task["phase"] in ["run", "check"] and task["case"] in [1, 2, 3]:
                assert pruned["results"][node]["skipped"]
                assert not list(self.objects(f"workers/attempts/{pruned['receipt']['run']}/{node}/"))
        self.report["checks"]["failed_parent_prunes_only_descendants_without_VM_execution"] = True
        self.cleanup(pruned)
        self.wait(self.only_one_ready, 600, "Excess VMs retired through declared policy")
        first = self.complete(self.submit(body(self.contest, "reuse-A", [100])))
        self.cleanup(first)
        second = self.complete(self.submit(body(self.contest, "reuse-B", [100])))
        self.cleanup(second)
        first_workers = {r["worker"] for r in first["results"].values()}
        second_workers = {r["worker"] for r in second["results"].values()}
        assert len(first_workers) == 1 and first_workers == second_workers
        self.report["checks"]["same_warm_VM_reused_after_verified_cleanup"] = True
        for phase in ["run", "after_result"]:
            request = "host-loss-" + phase
            data = body(self.contest, request, [8000 if phase == "run" else 100], inject_host_loss=True, host_loss_phase=phase)
            receipt = self.submit(data)
            state = self.complete(receipt, timeout=720)
            assert state["view"]["verdict"] == "AC" and self.api("POST", data) == receipt
            fault = self.read("workers/faults/" + receipt["run"] + ".json")
            if phase == "run":
                assert fault["docker_running"]
                assert state["results"][fault["node"]]["worker"] != fault["worker"]
            else:
                assert fault["result_saved"]
                assert state["results"][fault["node"]]["worker"] == fault["worker"]
                task = dict(state["jobs"])[fault["node"]]
                ref = state["results"][fault["node"]]["artifacts"][task["outputs"][0]]
                assert len(list(self.objects(ref["key"].rsplit('/', 2)[0] + '/'))) == 1
            attempts = [self.read(v["Key"]) for v in self.objects(f"workers/attempts/{receipt['run']}/{fault['node']}/")]
            assert max(v["receive_count"] for v in attempts) >= 2
            self.report["scenarios"][request].update(fault=fault, receive_counts=[v["receive_count"] for v in attempts])
            self.report["checks"][request + "_same_run_recovered"] = True
        receipt = self.submit(body(self.contest, "poison", [100], inject_poison=True))
        poison = self.complete(receipt, timeout=240)
        assert poison["view"]["status"] == "infrastructure_error"
        archives = [self.read(v["Key"]) for v in self.objects("deadletters/" + receipt["run"] + "/")]
        assert len(archives) == 1 and archives[0]["origin"] == "node_dlq"
        attempts = [self.read(v["Key"]) for v in self.objects("workers/attempts/" + receipt["run"] + "/")]
        self.report["scenarios"]["poison"]["receive_counts"] = sorted(v["receive_count"] for v in attempts)
        assert max(v["receive_count"] for v in attempts) >= 5
        recovery = self.submit({"contest": self.contest, "request_id": "operator-rejudge", "rejudge_of": "poison"})
        recovered = self.complete(recovery)
        assert recovered["view"]["verdict"] == "AC" and recovery["run"] != receipt["run"] and recovery["submission"] == receipt["submission"]
        self.cleanup(recovered)
        self.report["checks"]["bounded_retries_DLQ_quarantine_terminal_IE_and_explicit_rejudge"] = True
        def sleep_accepted():
            error, _ = self.capacity("sleep")
            return not error
        self.wait(sleep_accepted, 180, "Idle drain requested")
        retired = self.wait(lambda: [h["worker"] for h in self.sample(force=True)["hosts"] if h["state"] in ["draining", "drained"]],
                            120, "Retirement fence observed")
        rebound = self.complete(self.submit(body(self.contest, "arrival-during-drain", [100])), timeout=720)
        assert rebound["view"]["verdict"] == "AC"
        assert not ({r["worker"] for r in rebound["results"].values()} & set(retired))
        self.cleanup(rebound)
        self.report["checks"]["new_submission_during_irreversible_drain_runs_on_safe_worker"] = True
        self.wait(sleep_accepted, 180, "Final idle drain requested")
        self.wait(lambda: self.sample(force=True)["desired"] == 0 and self.sample()["physical_members"] == 0,
                  600, "Zero instances observed")
        self.report["checks"]["final_ASG_instances_zero"] = True
        by_worker = collections.defaultdict(list)
        all_intervals = []
        for state in self.states:
            for result in state["results"].values():
                if "execution" in result:
                    interval = (result["execution"]["started_ns"], result["execution"]["finished_ns"])
                    by_worker[result["worker"]].append(interval)
                    all_intervals.append(interval)
        for intervals in by_worker.values():
            values = sorted(intervals)
            assert all(a[1] <= b[0] for a, b in zip(values, values[1:])), "two executions overlapped in one VM"
        for sample in self.report["samples"]:
            for host in sample["hosts"]:
                if not host["protected"]:
                    assert not any(a <= sample["at"] * 1e9 < b for a, b in by_worker[host["worker"]]), "unprotected active execution"
        active = peak = 0
        for _, change in sorted([(a, 1) for a, _ in all_intervals] + [(b, -1) for _, b in all_intervals]):
            active += change
            peak = max(peak, active)
        assert peak <= 5
        self.report["observed_peak_parallel_executions"] = peak
        self.report["checks"].update(saved_execution_intervals_per_agent_boot_nonoverlapping=True,
            saved_intervals_observed_parallelism_at_most_five=True, sampled_active_execution_intervals_kept_scale_in_protection=True)
        for name in ["ControlDLQ", "NodeDLQ"]:
            values = self.sqs.get_queue_attributes(QueueUrl=self.outputs[name], AttributeNames=["ApproximateNumberOfMessages",
                "ApproximateNumberOfMessagesNotVisible", "ApproximateNumberOfMessagesDelayed"])["Attributes"]
            assert all(int(v) == 0 for v in values.values())
        self.report["checks"]["no_unhandled_DLQ_messages_at_end"] = True
        self.report["finished_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        self.save()
        print("Measured scaling scenarios complete; inspect each acceptance result", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--resume-after-forecast", action="store_true")
    args = parser.parse_args()
    experiment = Experiment(args.stack, args.report, args.resume_after_forecast)
    try:
        experiment.run()
    except Exception as exc:
        experiment.report["failure_type"] = type(exc).__name__
        experiment.save()
        raise


if __name__ == "__main__":
    main()
