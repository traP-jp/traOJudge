#!/usr/bin/env python3
"""Real AWS proof. Discovers endpoints in memory; reports contain no AWS IDs/URLs."""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import sys
import statistics
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from zoneinfo import ZoneInfo

import boto3
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest

REGION = "ap-northeast-1"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=1500)
    args = parser.parse_args()
    session = boto3.Session(region_name=REGION)
    cfn = session.client("cloudformation")
    outputs = {o["OutputKey"]: o["OutputValue"] for o in cfn.describe_stacks(StackName=args.stack)["Stacks"][0]["Outputs"]}
    s3, sqs, batch = [session.client(name) for name in ["s3", "sqs", "batch"]]
    is_pool = "AutoScalingGroup" in outputs
    contest = "e2e-" + uuid.uuid4().hex[:12]
    report = {"scenario": "native-compile-run-check", "region": REGION,
              "started_at": datetime.datetime.now(ZoneInfo("Asia/Tokyo")).isoformat(),
              "checks": {}, "cases": {}, "observations": []}
    if is_pool:
        from pool_checks import wait_ready
        wait_ready(session, outputs, report)

    def api(method, body=None, query=None):
        url = outputs["ApiUrl"] + ("?" + urllib.parse.urlencode(query) if query else "")
        data = json.dumps(body).encode() if body is not None else b""
        request = AWSRequest(method=method, url=url, data=data, headers={"Content-Type": "application/json", "X-Amz-Content-Sha256": hashlib.sha256(data).hexdigest()})
        SigV4Auth(session.get_credentials().get_frozen_credentials(), "lambda", REGION).add_auth(request)
        try:
            with urllib.request.urlopen(urllib.request.Request(url, data=data if method == "POST" else None,
                headers=dict(request.headers.items()), method=method), timeout=35) as response:
                return json.load(response)
        except urllib.error.HTTPError as exc:
            # Preserve error categories, never print a signed URL or AWS request body.
            text = exc.read().decode()
            raise RuntimeError(f"API HTTP {exc.code}: {text[:300]}") from None

    def submit(name, source, **extra):
        if name == "TLE":
            extra["limits"] = {"wall_seconds": 8, "cpu_seconds": 1, "memory_mb": 128}
        body = {"contest": contest, "request_id": name, "source": source, "language": "c", "tests": [
            {"input": "1 2\n", "expected": "3\n"}, {"input": "4 5\n", "expected": "9\n"},
            {"input": "-1 1\n", "expected": "0\n"}], **extra}
        return body, api("POST", body)

    sources = {
        "AC": r'''
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <errno.h>
int main(void) {
  if (getuid()!=65532 || prctl(PR_GET_NO_NEW_PRIVS,0,0,0,0)!=1) return 101;
  if (getenv("MANIFEST_URL") || getenv("AWS_ACCESS_KEY_ID") || getenv("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI")) return 102;
  errno=0; if(socket(AF_INET,SOCK_STREAM,0)!=-1 || errno!=EPERM) return 103;
  if(fopen("inputs/expected","r") || fopen("/opt/escape","w")) return 104;
  void *p=malloc(512UL*1024*1024); if(p) return 105;
  pid_t children[40]; int count=0;
  for(;count<40;++count) { pid_t p=fork(); if(p==0)_exit(0); if(p<0)break; children[count]=p; }
  if(count==40) return 107;
  for(int i=0;i<count;++i) waitpid(children[i],0,0);
  char path[80]; snprintf(path,sizeof(path),"/proc/%d/environ",getppid());
  if(fopen(path,"r")) return 108;
  long a,b; if(scanf("%ld%ld",&a,&b)!=2) return 106;
  if(a==1) sleep(1);
  printf("%ld\n",a+b); return 0;
}''',
        "WA": '#include <stdio.h>\nint main(){long a,b;scanf("%ld%ld",&a,&b);printf("%ld\\n",a-b);}',
        "TLE": 'int main(){volatile unsigned long n=0;for(;;)++n;}',
        "RE": '#include <stdlib.h>\nint main(){abort();}',
        "CE": 'this is not valid C;',
        "OLE": '#include <stdio.h>\nint main(){for(;;)puts("01234567890123456789012345678901234567890123456789");}',
    }
    receipts, bodies = {}, {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        futures = {pool.submit(submit, name, source): name for name, source in sources.items()}
        for future in concurrent.futures.as_completed(futures):
            name = futures[future]
            bodies[name], receipts[name] = future.result()
            print(f"accepted {name}: submission={receipts[name]['submission']}", flush=True)
    duplicate = api("POST", bodies["AC"])
    assert duplicate == receipts["AC"], "idempotency changed acceptance identity"
    report["checks"]["idempotent_acceptance"] = True
    ordered = sorted(receipts.values(), key=lambda v: v["submission"])
    assert [r["submission"] for r in ordered] == list(range(1, len(sources) + 1))
    assert [r["accepted_at"] for r in ordered] == sorted(r["accepted_at"] for r in ordered)
    report["checks"]["concurrent_ordering"] = True

    # Overwrite latest source; execution must use the accepted object version.
    ddb = session.client("dynamodb")
    original = json.loads(ddb.get_item(TableName=outputs["Table"], Key={"pk": {"S": f"RECEIPT#{contest}#AC"}}, ConsistentRead=True)["Item"]["value"]["S"])
    s3.put_object(Bucket=outputs["Bucket"], Key=original["artifacts"]["source"]["key"], Body=b"invalid replacement source")
    report["checks"]["source_overwrite_injected"] = True
    fault_contest = contest
    fault_body = {**bodies["AC"], "contest": fault_contest, "request_id": "durable", "crash_after_checkpoint": True,
                  "tests": [{"input": "1 2\n", "expected": "3\n"}]}
    receipts["durable"] = api("POST", fault_body)
    expected = {**{name: name for name in sources}, "durable": "AC"}
    start = time.monotonic()
    finished, latest = {}, {}
    while len(finished) < len(receipts):
        if time.monotonic() - start > args.timeout:
            raise TimeoutError(f"E2E deadline exceeded; completed={sorted(finished)}")
        for name, receipt in list(receipts.items()):
            if name in finished:
                continue
            if name == "RE" and time.monotonic() - start < 90:
                continue  # The workload must continue with this reader disconnected.
            view = api("GET", query={"contest": receipt["contest"], "request_id": receipt["request_id"]})
            progress = view["completed_cases"]
            assert progress >= latest.get(name, 0), "progress regressed"
            latest[name] = progress
            report["observations"].append({"case": name, "seconds": round(time.monotonic() - start, 2),
                "status": view["status"], "completed_cases": progress})
            if view["status"] == "infrastructure_error":
                raise AssertionError(f"{name}: infrastructure failure")
            if view["status"] == "completed":
                assert view["verdict"] == expected[name], f"{name}: unexpected verdict {view['verdict']}"
                if name != "CE":
                    assert progress == view["total_cases"], "some cases did not finish"
                    history = view["history"]
                    assert len({v["case"] for v in history}) == len(history)
                    assert [v["completed_cases"] for v in history] == list(range(1, len(history) + 1))
                finished[name] = view
                report["cases"][name] = {"verdict": view["verdict"], "completed_cases": progress,
                    "total_cases": view["total_cases"], "seconds": round(time.monotonic() - start, 2)}
                print(f"completed {name}: {view['verdict']} {progress}/{view['total_cases']}", flush=True)
        if len(finished) == len(sources) + 1 and "rejudge" not in receipts:
            receipt = api("POST", {"contest": contest, "request_id": "rejudge", "rejudge_of": "AC"})
            assert receipt["submission"] == receipts["AC"]["submission"] and receipt["run"] != receipts["AC"]["run"]
            receipts["rejudge"] = receipt
            expected["rejudge"] = "AC"
        time.sleep(4)

    report["checks"].update(native_execution=True, seccomp_network_denied=True, workload_identity_unprivileged=True,
        workload_credentials_absent=True, address_space_limit=True, read_only_rootfs=True,
        source_version_pinned=True, all_cases_continue_after_WA_TLE_RE=True, checkpoint_stop_recovered=True,
        process_count_limit=True, supervisor_environment_unreadable=True, output_size_limit=True,
        progress_survives_disconnected_reader=True, rejudge_separate_run=True)
    tle = receipts["TLE"]
    state = json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=f"checkpoints/{contest}/{tle['submission']}/{tle['run']}.json")["Body"].read())
    run_results = [r for r in state["results"].values() if r.get("exit_code") == -24]
    assert len(run_results) == 3 and all(not r["timed_out"] for r in run_results), "CPU limit was not observed independently of wall timeout"
    report["checks"]["cpu_time_limit"] = True
    # Deliver an old completion again; completed state must remain unchanged.
    ac = receipts["AC"]
    prefix = f"results/{contest}/{ac['submission']}/{ac['run']}/"
    objects = s3.list_objects_v2(Bucket=outputs["Bucket"], Prefix=prefix)["Contents"]
    obj = objects[0]
    result = json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=obj["Key"])["Body"].read())
    sqs.send_message(QueueUrl=outputs["ControlQueue"], MessageGroupId=ac["run"], MessageDeduplicationId=uuid.uuid4().hex,
        MessageBody=json.dumps({"kind": "result", "contest": contest, "receipt": original, "node": result["node"]}))
    time.sleep(8)
    reread = api("GET", query={"contest": contest, "request_id": "AC"})
    assert reread == finished["AC"]
    assert api("GET", query={"contest": contest, "request_id": "rejudge"}) == finished["rejudge"]
    report["checks"].update(duplicate_completion_idempotent=True, old_run_does_not_change_rejudge=True)
    for name in [key for key in ["ControlDLQ", "NodeDLQ", "ResultDLQ"] if key in outputs]:
        attrs = sqs.get_queue_attributes(QueueUrl=outputs[name], AttributeNames=["ApproximateNumberOfMessages"])["Attributes"]
        assert int(attrs["ApproximateNumberOfMessages"]) == 0, f"{name} is not empty"
    report["checks"]["dead_letter_queues_empty"] = True
    if is_pool:
        assert all(result.get("isolation", {}).get("runtime") == "runsc" and result["isolation"]["network"] == "none"
                   and result["isolation"]["read_only_rootfs"] for result in state["results"].values())
        report["checks"]["fresh_offline_gvisor_sandboxes"] = True
        from pool_checks import performance, sleep_pool
        performance(api, report)
        sleep_pool(session, outputs, report)
    else:
        definition = batch.describe_job_definitions(jobDefinitions=[outputs["JobDefinition"]])["jobDefinitions"][0]["containerProperties"]
        assert not definition.get("jobRoleArn") and definition["readonlyRootFilesystem"]
        report["checks"]["deployed_task_has_no_IAM_role"] = True
        # Observe the idle execution plane, not just a scale-to-zero declaration.
        idle_deadline = time.monotonic() + 240
        while True:
            counts = {status: len(batch.list_jobs(jobQueue=outputs["JobQueue"], jobStatus=status).get("jobSummaryList", []))
                      for status in ["SUBMITTED", "PENDING", "RUNNABLE", "STARTING", "RUNNING"]}
            if sum(counts.values()) == 0:
                break
            if time.monotonic() > idle_deadline:
                raise TimeoutError("Batch execution did not return to zero")
            time.sleep(10)
        report["checks"]["idle_execution_jobs_zero"] = True
        report["idle_job_counts"] = counts
        jobs = []
        for status in ["SUCCEEDED", "FAILED"]:
            for page in batch.get_paginator("list_jobs").paginate(jobQueue=outputs["JobQueue"], jobStatus=status):
                ids = [job["jobId"] for job in page.get("jobSummaryList", [])]
                for start_index in range(0, len(ids), 100):
                    jobs.extend(batch.describe_jobs(jobs=ids[start_index:start_index + 100])["jobs"])

        def distribution(values):
            ordered_values = sorted(values)
            return {"median": round(statistics.median(ordered_values), 3),
                    "p95": round(ordered_values[int((len(ordered_values)-1)*0.95)], 3),
                    "max": round(max(ordered_values), 3)} if ordered_values else {}

        report["batch"] = {"tasks": len(jobs), "failed_tasks": sum(j["status"] == "FAILED" for j in jobs),
            "accept_to_running_seconds": distribution([(j["startedAt"]-j["createdAt"])/1000 for j in jobs if j.get("startedAt")]),
            "running_to_stopped_seconds": distribution([(j["stoppedAt"]-j["startedAt"])/1000 for j in jobs if j.get("startedAt") and j.get("stoppedAt")]),
            "compute_minimum_USD": round(len(jobs) * 0.03081 / 60, 6)}
    assert any(0 < v["completed_cases"] < 3 for v in report["observations"]), "no intermediate progress was observed"
    assert max(report["cases"][name]["seconds"] for name in sources) < 180, "other runs did not finish during the failed run's visibility interval"
    resources = cfn.describe_stack_resources(StackName=args.stack)["StackResources"]
    engine_logs = next(r["PhysicalResourceId"] for r in resources if r["ResourceType"] == "AWS::Logs::LogGroup" and r["LogicalResourceId"].startswith("EngineLogs"))
    fault_events = session.client("logs").filter_log_events(logGroupName=engine_logs,
        filterPattern='"injected stop after durable checkpoint, before dispatch"').get("events", [])
    assert fault_events, "the checkpoint-stop injection was not observed in AWS logs"
    report["checks"].update(intermediate_progress_observed=True, other_runs_unblocked_by_failure=True,
                           checkpoint_failure_observed_in_logs=True)
    for name, receipt in receipts.items():
        head = s3.head_object(Bucket=outputs["Bucket"], Key=f"checkpoints/{receipt['contest']}/{receipt['submission']}/{receipt['run']}.json")
        accepted_at = datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp()
        report["cases"][name]["accept_to_checkpoint_seconds"] = round(head["LastModified"].timestamp() - accepted_at, 3)
    report["duration_seconds"] = round(time.monotonic() - start, 2)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: {len(report['checks'])} checks; report={args.report}", flush=True)


if __name__ == "__main__":
    main()
