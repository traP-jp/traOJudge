#!/usr/bin/env python3
"""Exercise receive-before-drain on real ASG members, without changing capacity."""
import argparse
import collections
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import time

from run_scaling import Experiment
from workloads import body


class DrainExperiment(Experiment):
    def __init__(self, stack, report_path):
        super().__init__(stack, report_path)
        self.vm_aliases = {}
        self.next_vm_alias = 1
        self.raw_hosts = {}
        self.report["source_sha256"] = {
            str(p): hashlib.sha256(p.read_bytes()).hexdigest()
            for root in ["infra/runtime", "infra/pool", "infra/policy", "infra/bin", "infra/worker"]
            for p in sorted(Path(root).glob("*"))
            if p.is_file() and p.suffix in [".py", ".ts", ".json"]
        }

    def sample(self, force=False):
        if not force and self.report["samples"] and time.monotonic() - self.last_sample < 3:
            return self.report["samples"][-1]
        group = self.scaling.describe_auto_scaling_groups(AutoScalingGroupNames=[self.outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
        members = {v["InstanceId"]: v for v in group["Instances"]}
        assert group["MaxSize"] == 5
        hosts = {}
        for obj in self.objects("workers/", delimiter="/"):
            host = self.read(obj["Key"])
            instance = host["instance_id"]
            if instance not in members or time.time() - host["heartbeat_at"] > 20:
                continue
            if instance not in hosts or hosts[instance]["heartbeat_at"] < host["heartbeat_at"]:
                hosts[instance] = host
        self.raw_hosts = hosts
        public = []
        for instance, host in hosts.items():
            if instance not in self.vm_aliases:
                self.vm_aliases[instance] = "vm-%02d" % self.next_vm_alias
                self.next_vm_alias += 1
            alias = self.vm_aliases[instance]
            protected = members[instance]["ProtectedFromScaleIn"]
            assert host["executions_per_instance"] == 1 and host["active"] in [0, 1]
            # Sampled observations are not atomic; the local admission fence is
            # separately checked by pure tests. Keep all observed values.
            public.append({"vm": alias, **{k: host.get(k) for k in [
                "worker", "state", "active", "polling", "poll_started_at", "admission", "drain_id", "heartbeat_at"]},
                "protected": protected})
        snapshot = {"at": time.time(), "desired": group["DesiredCapacity"], "physical_members": len(members), "hosts": public}
        self.report["samples"].append(snapshot)
        self.last_sample = time.monotonic()
        self.save()
        return snapshot

    def run(self):
        assert self.report["policy_profile"] == "prewarmed"
        self.inspect_declarations()
        def polling_five():
            sample = self.sample(force=True)
            hosts = sample["hosts"]
            return sample if (len(hosts) == 5 and all(h["state"] == "ready" and h["protected"] and
                h["polling"] and not h["active"] for h in hosts) and
                sum(time.time() - h["poll_started_at"] < 4 for h in hosts) >= 2) else None
        initial = self.wait(polling_five, 360, "Five protected VMs with reserved receives observed")
        initial_workers = {h["worker"] for h in initial["hosts"]}
        error, _ = self.capacity("sleep")
        assert not error
        # Observe the interleaving, rather than assuming the command arrived.
        def draining_poll():
            sample = self.sample(force=True)
            return sample if any(h["worker"] in initial_workers and h["state"] == "draining" and
                h["polling"] and h["protected"] for h in sample["hosts"]) else None
        observed = self.wait(draining_poll, 20, "Drain arrived while a protected receive was outstanding")
        self.report["drain_with_pending_poll_observed_at"] = observed["at"]
        requests = [body(self.contest, "drain-arrival-%d" % i, [500, 500]) for i in range(5)]
        with ThreadPoolExecutor(max_workers=5) as executor:
            receipts = list(executor.map(lambda request: self.api("POST", request), requests))
        for request, receipt in zip(requests, receipts):
            self.report["scenarios"][request["request_id"]] = {"receipt": receipt}
        self.save()
        for receipt in receipts:
            state = self.complete(receipt, timeout=600)
            assert state["view"]["verdict"] == "AC" and state["view"]["completed_cases"] == 2
            self.cleanup(state)
        attempts = [self.read(o["Key"]) for o in self.objects("workers/attempts/")]
        reserved = [a for a in attempts if a.get("admission", {}).get("mode") == "reserved_before_drain"]
        self.report["attempts"] = attempts
        self.report["checks"]["reserved_receive_actually_executed_after_drain"] = bool(reserved)
        assert reserved, "targeted interleaving was not exercised"
        self.report["checks"]["all_five_submissions_completed_AC_during_retirement"] = True
        self.report["receive_count_histogram"] = dict(collections.Counter(a["receive_count"] for a in attempts))
        intervals = collections.defaultdict(list)
        worker_vm = {}
        for obj in self.objects("workers/", delimiter="/"):
            h = self.read(obj["Key"])
            if h["instance_id"] not in self.vm_aliases:
                self.vm_aliases[h["instance_id"]] = "vm-%02d" % self.next_vm_alias
                self.next_vm_alias += 1
            alias = self.vm_aliases[h["instance_id"]]
            worker_vm[h["worker"]] = alias
        for state in self.states:
            for node, result in state["results"].items():
                if "execution" in result:
                    intervals[worker_vm[result["worker"]]].append({"node": node, **result["execution"]})
        for values in intervals.values():
            ordered = sorted(values, key=lambda v: v["started_ns"])
            assert all(a["finished_ns"] <= b["started_ns"] for a, b in zip(ordered, ordered[1:]))
        self.report["saved_execution_intervals_by_VM"] = intervals
        self.report["checks"]["saved_intervals_per_physical_VM_do_not_overlap"] = True
        # Original retired boots must never resume; only their already admitted
        # receives may finish. The subsequent DAG nodes use replacement VMs.
        for a in attempts:
            if a["worker"] in initial_workers:
                assert a["admission"]["poll_started_at"] < observed["at"]
        self.report["checks"]["retired_boots_only_execute_preexisting_poll_reservations"] = True
        self.wait(lambda: len([h for h in self.sample(force=True)["hosts"] if h["state"] == "ready" and h["protected"]]) == 5,
                  480, "Five replacement VMs ready; original boots remained retired")
        self.finish()

    def finish(self):
        # Admission is quiesced now. Explicit sleep is an administrative shutdown,
        # not an atomic guarantee against arbitrary new external submissions.
        for _ in range(20):
            error, _ = self.capacity("sleep")
            if not error:
                break
            time.sleep(3)
        assert not error
        self.wait(lambda: self.sample(force=True)["desired"] == 0 and self.sample()["physical_members"] == 0,
                  480, "All VMs retired through declared policy")
        dlqs = []
        for _ in range(2):
            counts = {}
            for name in ["NodeDLQ", "ControlDLQ"]:
                values = self.sqs.get_queue_attributes(QueueUrl=self.outputs[name], AttributeNames=[
                    "ApproximateNumberOfMessages", "ApproximateNumberOfMessagesNotVisible", "ApproximateNumberOfMessagesDelayed"])["Attributes"]
                counts[name] = {k: int(v) for k, v in values.items()}
            assert all(v == 0 for attrs in counts.values() for v in attrs.values())
            dlqs.append({"at": time.time(), "counts": counts})
            time.sleep(5)
        self.report["settled_DLQ_samples"] = dlqs
        self.report["controller_observations"] = [self.read(o["Key"]) for o in self.objects("workers/scaling/")]
        self.report["checks"].update(final_ASG_desired_and_members_zero=True, settled_DLQs_zero=True)
        self.report["measurement_limits"] = ["Heartbeat and ASG samples are non-atomic, every approximately three seconds.",
            "Saved canonical execution intervals omit any killed or losing duplicate attempt.",
            "This targeted trial tests healthy drain/poll admission, not application-health watchdog recovery."]
        self.report["finished_at_unix"] = time.time()
        self.save()
        print("Focused drain/poll trial passed", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    experiment = DrainExperiment(args.stack, args.report)
    try:
        experiment.run()
    except BaseException as exc:
        experiment.report["failure_type"] = type(exc).__name__
        experiment.save()
        print("Trial failed: " + type(exc).__name__, flush=True)
        raise


if __name__ == "__main__":
    main()
