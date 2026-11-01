#!/usr/bin/env python3
"""Real AWS failure scenario: sibling results after a run has terminated."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request
import uuid
from zoneinfo import ZoneInfo

import boto3
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest
from botocore.exceptions import ClientError


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    session = boto3.Session(region_name="ap-northeast-1")
    outputs = {v["OutputKey"]: v["OutputValue"] for v in session.client("cloudformation").describe_stacks(StackName=args.stack)["Stacks"][0]["Outputs"]}
    s3, sqs, batch, functions = [session.client(name) for name in ["s3", "sqs", "batch", "lambda"]]
    contest = "late-" + uuid.uuid4().hex[:12]
    is_pool = "AutoScalingGroup" in outputs
    began_at_zero = False
    if is_pool:
        group = session.client("autoscaling").describe_auto_scaling_groups(AutoScalingGroupNames=[outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
        began_at_zero = group["DesiredCapacity"] == 0 and not group["Instances"]
    started = time.monotonic()
    report = {"scenario": "late-sibling-after-infrastructure-failure", "checks": {},
              "started_at": datetime.datetime.now(ZoneInfo("Asia/Tokyo")).isoformat()}

    def api(method, body=None):
        url = outputs["ApiUrl"]
        if method == "GET":
            url += "?" + urllib.parse.urlencode({"contest": contest, "request_id": "late"})
        data = json.dumps(body).encode() if body else b""
        req = AWSRequest(method=method, url=url, data=data,
                         headers={"Content-Type": "application/json", "X-Amz-Content-Sha256": hashlib.sha256(data).hexdigest()})
        SigV4Auth(session.get_credentials().get_frozen_credentials(), "lambda", "ap-northeast-1").add_auth(req)
        with urllib.request.urlopen(urllib.request.Request(url, data=data if method == "POST" else None,
                headers=dict(req.headers.items()), method=method), timeout=35) as response:
            return json.load(response)

    def read(key):
        try:
            return json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=key)["Body"].read())
        except ClientError as exc:
            if exc.response["Error"]["Code"] in ["NoSuchKey", "404"]:
                return None
            raise

    def until(check, seconds=600):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            value = check()
            if value:
                return value
            time.sleep(2)
        raise TimeoutError("late-result proof exceeded deadline")

    accepted = api("POST", {"contest": contest, "request_id": "late",
        "source": '#include <stdio.h>\n#include <unistd.h>\nint main(){long a,b;scanf("%ld%ld",&a,&b);sleep(2);printf("%ld",a+b);}',
        "limits": {"wall_seconds": 10, "cpu_seconds": 2, "memory_mb": 128},
        "tests": [{"input": "1 2\n", "expected": "3\n"}, {"input": "4 5\n", "expected": "9\n"}]})
    checkpoint = f"checkpoints/{contest}/{accepted['submission']}/{accepted['run']}.json"
    if is_pool:
        from pool_checks import wait_ready
        wait_ready(session, outputs, report)
        report["cold_submission_to_ready_seconds"] = round(time.time() - datetime.datetime.fromisoformat(accepted["accepted_at"]).timestamp(), 3)
        if began_at_zero:
            report["checks"]["new_receipt_wakes_zero_pool"] = True

    def parallel_runs():
        state = read(checkpoint)
        if not state:
            return None
        runs = [(node, task) for node, task in state["pending"].items() if task["phase"] == "run" and node in state["sent"]]
        return (state, runs) if len(runs) == 2 else None

    state, runs = until(parallel_runs)
    receipt = state["receipt"]
    failed, sibling = runs[0][0], runs[1][0]
    prefix = f"results/{contest}/{accepted['submission']}/{accepted['run']}/"
    s3.put_object(Bucket=outputs["Bucket"], Key=prefix + failed + ".json", IfNoneMatch="*",
        Body=json.dumps({"contest": contest, "submission": accepted["submission"], "run": accepted["run"],
                         "node": failed, "infrastructure_error": "injected node infrastructure failure"}).encode())
    if is_pool:
        sqs.send_message(QueueUrl=outputs["ControlQueue"], MessageGroupId=receipt["run"], MessageDeduplicationId=uuid.uuid4().hex,
            MessageBody=json.dumps({"kind": "result", "contest": contest, "receipt": receipt, "node": failed}))
    terminal = until(lambda: (v if (v := api("GET"))["status"] == "infrastructure_error" else None))
    print("injected infrastructure failure: run terminated", flush=True)
    result = until(lambda: read(prefix + sibling + ".json"))
    assert result["exit_code"] == 0 and not result["timed_out"]
    # Explicit replay also verifies the exact handler response, without waiting
    # for five failed receives to appear in the DLQ.
    invocation = functions.invoke(FunctionName=outputs["EngineFunction"], Payload=json.dumps({"Records": [{"body": json.dumps({
        "kind": "result", "contest": contest, "receipt": receipt, "node": sibling})}]}).encode())
    invocation["Payload"].read()
    assert "FunctionError" not in invocation, "valid late completion was rejected"
    assert api("GET") == terminal, "late sibling changed terminal state"
    report["checks"].update(parallel_native_nodes_dispatched=True, infrastructure_failure_separate_from_verdict=True,
        native_sibling_result_arrived_after_termination=True, valid_late_result_acknowledged=True, terminal_state_unchanged=True)
    if is_pool:
        from pool_checks import sleep_pool
        sleep_pool(session, outputs, report)
    else:
        until(lambda: all(not batch.list_jobs(jobQueue=outputs["JobQueue"], jobStatus=status).get("jobSummaryList")
                          for status in ["SUBMITTED", "PENDING", "RUNNABLE", "STARTING", "RUNNING"]), seconds=240)
    for name in [key for key in ["ControlDLQ", "NodeDLQ", "ResultDLQ"] if key in outputs]:
        attributes = sqs.get_queue_attributes(QueueUrl=outputs[name], AttributeNames=["ApproximateNumberOfMessages"])["Attributes"]
        assert int(attributes["ApproximateNumberOfMessages"]) == 0
    report["checks"]["dead_letter_queues_empty"] = True
    if not is_pool:
        report["checks"]["idle_execution_jobs_zero"] = True
    report["duration_seconds"] = round(time.monotonic() - started, 2)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: {len(report['checks'])} late-result checks; report={args.report}", flush=True)


if __name__ == "__main__":
    main()
