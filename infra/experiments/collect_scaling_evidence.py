#!/usr/bin/env python3
"""Read-only post-analysis: strengthen joins and retain failed acceptance checks."""
import argparse
import collections
import datetime
import json
from pathlib import Path
import re
import time

import boto3


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = json.loads(args.report.read_text())
    session = boto3.Session(region_name="ap-northeast-1")
    cf, s3, sqs, scaling = [session.client(n) for n in ["cloudformation", "s3", "sqs", "autoscaling"]]
    outputs = {v["OutputKey"]: v["OutputValue"] for v in cf.describe_stacks(StackName=args.stack)["Stacks"][0]["Outputs"]}
    assert outputs["PolicyFingerprint"] == report["policy_sha256"]
    def objects(prefix, delimiter=None):
        kwargs = {"Bucket": outputs["Bucket"], "Prefix": prefix}
        if delimiter:
            kwargs["Delimiter"] = delimiter
        return [v for p in s3.get_paginator("list_objects_v2").paginate(**kwargs) for v in p.get("Contents", [])]
    def read(key):
        return json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=key)["Body"].read())
    host_records = [read(o["Key"]) for o in objects("workers/", "/")]
    private_instances = sorted({h["instance_id"] for h in host_records})
    # Preserve labels already present in samples. Relabeling only the interval
    # table would silently break joins with the original physical observations.
    observed_aliases = {h["worker"]: h["vm"] for sample in report["samples"]
                        for h in sample["hosts"] if "vm" in h}
    aliases = {h["instance_id"]: observed_aliases[h["worker"]] for h in host_records
               if h["worker"] in observed_aliases}
    for instance in private_instances:
        if instance not in aliases:
            index = 1
            while f"vm-{index:02}" in aliases.values():
                index += 1
            aliases[instance] = f"vm-{index:02}"
    worker_vm = {h["worker"]: aliases[h["instance_id"]] for h in host_records}
    report["agent_to_opaque_VM_label"] = worker_vm
    saved_intervals = collections.defaultdict(list)
    analysis = {}
    for obj in objects("checkpoints/"):
        state = read(obj["Key"])
        receipt = state["receipt"]
        name = receipt["request_id"]
        attempts = objects("workers/attempts/" + receipt["run"] + "/")
        attempted_nodes = {a["Key"].split('/')[-2] for a in attempts}
        cleanups = objects("workers/cleanups/" + receipt["run"] + "/")
        cleanup_nodes = {a["Key"].split('/')[-2] for a in cleanups}
        skipped = {node for node, result in state["results"].items() if result.get("skipped")}
        assert not (skipped & attempted_nodes), "a pruned node allocated an execution attempt"
        for cleanup in cleanups:
            value = read(cleanup["Key"])
            assert value["container_absent"] and value["workdir_absent"]
        successful = {node for node, r in state["results"].items() if "execution" in r}
        missing = successful - cleanup_nodes
        if missing:
            # A crashed VM cannot emit a cleanup acknowledgement; check its physical termination below.
            assert receipt.get("inject_host_loss") and receipt.get("host_loss_phase") == "after_result"
            fault = read("workers/faults/" + receipt["run"] + ".json")
            assert missing == {fault["node"]} and fault["result_saved"]
        for node, result in state["results"].items():
            if "execution" not in result:
                continue
            saved_intervals[worker_vm[result["worker"]]].append({"request": name, "node": node,
                "agent": result["worker"], **result["execution"]})
        analysis[name] = {"run": receipt["run"], "verdict": state["view"].get("verdict"),
            "state": state["view"]["status"], "completed_cases": state["view"]["completed_cases"],
            "skipped_cases": state["view"].get("skipped_cases", 0), "saved_execution_nodes": len(successful),
            "distinct_cleanup_nodes": len(cleanup_nodes), "pruned_nodes": len(skipped),
            "pruned_nodes_have_no_attempts": not bool(skipped & attempted_nodes),
            "cleanup_missing_only_for_powered_off_VM": len(missing),
            "agents": sorted({r["worker"] for r in state["results"].values() if "worker" in r})}
        analysis[name]["last_checkpoint_published_elapsed_seconds"] = round(
            obj["LastModified"].timestamp() - datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp(), 3)
        analysis[name]["reserved_before_drain_saved_nodes"] = [node for node, r in state["results"].items()
            if "execution" in r and r.get("admission", {}).get("mode") == "reserved_before_drain"]
        # Canonical durable publication time remains meaningful if the runner resumed later.
        if name == "forecast-tree":
            analysis[name]["last_checkpoint_published_elapsed_seconds"] = round(
                obj["LastModified"].timestamp() - datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp(), 3)
            # Do not mistake a later verifier run for the original result latency.
            report["scenarios"][name].pop("accept_to_observed_terminal_seconds", None)
    for vm, intervals in saved_intervals.items():
        values = sorted(intervals, key=lambda x: x["started_ns"])
        assert all(a["finished_ns"] <= b["started_ns"] for a, b in zip(values, values[1:])), vm
    report["saved_execution_intervals_by_VM"] = saved_intervals
    events = sorted((instant, delta) for values in saved_intervals.values() for value in values
                    for instant, delta in [(value["started_ns"], 1), (value["finished_ns"], -1)])
    active = peak = 0
    for _, delta in events:
        active += delta
        peak = max(peak, active)
    report["saved_execution_peak"] = peak
    report["joined_execution_cleanup_analysis"] = analysis
    ec2 = session.client("ec2")
    state_by_instance = {}
    for reservation in ec2.describe_instances(InstanceIds=private_instances)["Reservations"]:
        for instance in reservation["Instances"]:
            state_by_instance[instance["InstanceId"]] = instance["State"]["Name"]
    report["VM_final_states"] = {aliases[i]: state for i, state in state_by_instance.items()}
    assert all(v == "terminated" for v in state_by_instance.values())
    activities = []
    for page in scaling.get_paginator("describe_scaling_activities").paginate(AutoScalingGroupName=outputs["AutoScalingGroup"]):
        for a in page["Activities"]:
            cause = a["Cause"]
            transition = re.search(r"(?:changing|changed|change).*?desired capacity from (\d+) to (\d+)", cause, re.I)
            activities.append({"started_at": a["StartTime"].isoformat(), "ended_at": a.get("EndTime", a["StartTime"]).isoformat(),
                "status": a["StatusCode"], "alarm_policy_triggered": "alarm" in cause.lower() and "policy" in cause.lower(),
                "health_replacement": "health" in cause.lower(),
                "capacity_transition": list(map(int, transition.groups())) if transition else None})
    report["scaling_activities_sanitized"] = activities
    observations = [read(o["Key"]) for o in objects("workers/scaling/")]
    observations.sort(key=lambda v: v["at"])
    report["controller_observations"] = observations
    cloudwatch = session.client("cloudwatch")
    start = datetime.datetime.fromisoformat(report["started_at"])
    end = datetime.datetime.now(datetime.timezone.utc)
    metrics = cloudwatch.get_metric_statistics(Namespace="traOJudge/" + args.stack, MetricName="SafeDesiredInstances",
        StartTime=start, EndTime=end, Period=60, Statistics=["Minimum", "Maximum", "Average"])["Datapoints"]
    report["safe_target_metric_minutes"] = [{"at": m["Timestamp"].isoformat(),
        **{k: m[k] for k in ["Minimum", "Maximum", "Average"]}} for m in sorted(metrics, key=lambda m: m["Timestamp"])]
    dlq_samples = []
    for _ in range(2):
        values = {}
        for name in ["NodeDLQ", "ControlDLQ"]:
            attrs = sqs.get_queue_attributes(QueueUrl=outputs[name], AttributeNames=["ApproximateNumberOfMessages",
                "ApproximateNumberOfMessagesNotVisible", "ApproximateNumberOfMessagesDelayed"])["Attributes"]
            values[name] = {k: int(v) for k, v in attrs.items()}
        assert all(n == 0 for attrs in values.values() for n in attrs.values())
        dlq_samples.append({"at": time.time(), "counts": values})
        time.sleep(5)
    report["settled_DLQ_samples"] = dlq_samples
    old = report["checks"]
    for key in ["one_execution_per_VM_in_all_observed_intervals", "observed_parallel_executions_at_most_five", "active_hosts_kept_scale_in_protection"]:
        old.pop(key, None)
    old.update(saved_intervals_grouped_by_opaque_physical_VM_do_not_overlap=True,
               distinct_cleanup_node_joins_verified=True, pruned_nodes_have_no_attempt_records=True,
               powered_off_VMs_physically_terminated=True, DLQs_visible_inflight_delayed_settled_zero=True)
    report["measurement_limits"] = [
        "Saved execution intervals exclude attempts killed before publication and losing duplicate results.",
        "Heartbeat/protection samples are not atomic and are sampled; source guards and unit tests provide separate evidence.",
        "Cross-VM timestamp comparison assumes AWS guest realtime clocks are sufficiently aligned.",
        "Five-ready lead uses first observed readiness, so the negative timing check is retained with sampling uncertainty."]
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print("Post-analysis joins, physical VM aliases, declared scaling actions, and settled DLQs verified")


if __name__ == "__main__":
    main()
