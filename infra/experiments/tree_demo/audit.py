#!/usr/bin/env python3
"""Read-only joins for the live recording, followed by graceful pool shutdown."""
import argparse
import collections
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_scaling import Experiment


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = json.loads(args.browser.read_text())
    aws = Experiment(args.stack, Path("work/tree-demo-audit-progress.json"))
    assert aws.outputs["PolicyFingerprint"] == report["policy_sha256"]
    aws.inspect_declarations()
    report["source_base_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    report["deployed_source_sha256"] = {
        str(p): hashlib.sha256(p.read_bytes()).hexdigest()
        for root in ["infra/runtime", "infra/worker", "infra/pool", "infra/bin", "infra/policy"]
        for p in sorted(Path(root).glob("*")) if p.is_file() and p.suffix in [".py", ".ts", ".json"]}
    host_records = [aws.read(v["Key"]) for v in aws.objects("workers/", delimiter="/")]
    private_vms = sorted({h["instance_id"] for h in host_records})
    labels = {v: f"vm-{i+1:02}" for i, v in enumerate(private_vms)}
    worker_vms = {h["worker"]: labels[h["instance_id"]] for h in host_records}
    report["worker_to_opaque_VM_label"] = worker_vms
    intervals = collections.defaultdict(list)
    scenarios = {}
    for receipt in report["receipts"]:
        state = aws.read(f"checkpoints/{receipt['contest']}/{receipt['submission']}/{receipt['run']}.json")
        assert state["receipt"]["test_parents"] == report["test_parents"]
        tasks = dict(state["jobs"])
        names = {node: task["name"] for node, task in tasks.items()}
        by_name = {name: node for node, name in names.items()}
        results = {v["Key"].rsplit('/', 1)[1][:-5]: aws.read(v["Key"])
            for v in aws.objects(f"results/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/")}
        attempts = []
        for obj in aws.objects(f"workers/attempts/{receipt['run']}/"):
            value = aws.read(obj["Key"])
            value["node_name"] = names[obj["Key"].split('/')[-2]]
            attempts.append(value)
        cleanups = list(aws.objects(f"workers/cleanups/{receipt['run']}/"))
        cleanup_nodes = {v["Key"].split('/')[-2] for v in cleanups}
        # Terminal WA is reported before the other already-dispatched run finishes.
        # Wait for that run's cleanup separately from the pipeline's terminal view.
        expected = set(results)
        for _ in range(30):
            cleanups = list(aws.objects(f"workers/cleanups/{receipt['run']}/"))
            cleanup_nodes = {v["Key"].split('/')[-2] for v in cleanups}
            if expected <= cleanup_nodes:
                break
            time.sleep(1)
        assert expected <= cleanup_nodes
        for obj in cleanups:
            value = aws.read(obj["Key"])
            assert value["container_absent"] and value["workdir_absent"]
        for node, result in results.items():
            assert result["isolation"] == {"runtime": "runsc", "network": "none", "read_only_rootfs": True}
            intervals[worker_vms[result["worker"]]].append({"request": receipt["request_id"],
                "node_name": names[node], **result["execution"]})
        issued = {names[n] for n in state["sent"]}
        actual = {names[n] for n in results}
        if receipt["request_id"] == "non-ac":
            assert state["view"]["verdict"] == "WA" and state["view"]["pipeline_state"] == "cancelled"
            assert state["view"]["nodes"][by_name["check-0"]] == "failed"
            assert issued == actual == {"compile", "run-0", "run-4", "check-0"}
            assert {a["node_name"] for a in attempts} == issued
            assert all(state["view"]["nodes"][n] == "waiting" for n in tasks if names[n] not in issued)
            assert results[by_name["check-0"]]["node_output"]["verdict"] == "WA"
            assert results[by_name["check-0"]]["execution"]["finished_ns"] < results[by_name["run-4"]]["execution"]["finished_ns"]
            assert state["view"]["completed_cases"] == 1
        else:
            assert state["view"]["verdict"] == "AC" and state["view"]["pipeline_state"] == "completed"
            assert len(issued) == len(actual) == 14
            assert state["view"]["completed_cases"] == 6
            assert state["view"]["summary"]["case_verdicts"] == ["AC"] * 6
        scenarios[receipt["request_id"]] = {"receipt": receipt, "final_view": state["view"],
            "issued_node_names": sorted(issued), "saved_result_node_names": sorted(actual),
            "engine_states_by_name": {names[n]: v for n, v in state["view"]["nodes"].items()},
            "attempts": attempts, "cleanup_distinct_nodes": len(cleanup_nodes),
            "external_execution_results": {names[n]: {k: r[k] for k in ["worker", "execution", "exit_code", "node_output"]} for n, r in results.items()}}
        print("Verified " + receipt["request_id"], flush=True)
    for values in intervals.values():
        ordered = sorted(values, key=lambda v: v["started_ns"])
        assert all(a["finished_ns"] <= b["started_ns"] for a, b in zip(ordered, ordered[1:]))
    active = peak = 0
    for _, delta in sorted((instant, delta) for values in intervals.values() for v in values
            for instant, delta in [(v["started_ns"], 1), (v["finished_ns"], -1)]):
        active += delta
        peak = max(peak, active)
    assert peak <= 5
    report.update(scenarios=scenarios, saved_execution_intervals_by_VM=intervals, saved_execution_peak=peak,
        checks={"non_AC_mapped_to_real_Engine_job_failed": True,
            "no_future_nodes_dispatched_after_pipeline_failure": True,
            "already_dispatched_external_run_finished_after_failed_check": True,
            "all_AC_tree_reached_summary": True, "distinct_cleanup_joins_verified": True,
            "saved_intervals_per_VM_do_not_overlap": True})
    report["measurement_limits"] = ["Two deliberately timed C17 fixture submissions with five initially warm VMs; not a latency benchmark or predictive-scaling trial.",
        "The cancelled Engine ignores late completion; the live adapter observes its durable external result separately.",
        "Saved canonical intervals exclude killed or losing duplicate attempts. Cross-VM ordering assumes aligned guest clocks.",
        "Browser states come from live snapshot polling; intermediate revisions may be missed."]
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    def asleep():
        error, _ = aws.capacity("sleep")
        return not error
    aws.wait(asleep, 180, "Idle pool drain requested")
    aws.wait(lambda: aws.sample(force=True)["desired"] == 0 and aws.sample()["physical_members"] == 0,
             480, "All VMs terminated before CloudFormation destroy")
    counts = {}
    for name in ["NodeDLQ", "ControlDLQ"]:
        values = aws.sqs.get_queue_attributes(QueueUrl=aws.outputs[name], AttributeNames=["ApproximateNumberOfMessages",
            "ApproximateNumberOfMessagesNotVisible", "ApproximateNumberOfMessagesDelayed"])["Attributes"]
        counts[name] = {k: int(v) for k, v in values.items()}
    assert all(v == 0 for attrs in counts.values() for v in attrs.values())
    report.update(final_DLQ_counts=counts, pool_desired_and_members_zero=True,
        audit_completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print("Recording audit complete; stack can now be destroyed", flush=True)


if __name__ == "__main__":
    main()
