#!/usr/bin/env python3
"""Real host loss: power off a managed test VM before any result/queue ACK."""
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

from pool_checks import wait_ready, sleep_pool


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    session = boto3.Session(region_name="ap-northeast-1")
    outputs = {v["OutputKey"]: v["OutputValue"] for v in session.client("cloudformation").describe_stacks(StackName=args.stack)["Stacks"][0]["Outputs"]}
    scaling = session.client("autoscaling")
    s3, sqs = session.client("s3"), session.client("sqs")
    report = {"scenario": "host-poweroff-before-result-and-queue-ack", "checks": {},
        "started_at": datetime.datetime.now(ZoneInfo("Asia/Tokyo")).isoformat()}
    wait_ready(session, outputs, report)
    def instances():
        group = scaling.describe_auto_scaling_groups(AutoScalingGroupNames=[outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
        return {v["InstanceId"] for v in group["Instances"]}
    original_instances = instances()
    assert len(original_instances) == 1
    contest = "host-" + uuid.uuid4().hex[:12]
    body = {"contest": contest, "request_id": "host-loss", "inject_host_loss": True,
        "source": '#include <stdio.h>\nint main(){long a,b;scanf("%ld%ld",&a,&b);printf("%ld",a+b);}',
        "tests": [{"input": "1 2\n", "expected": "3\n"}, {"input": "4 5\n", "expected": "9\n"},
                  {"input": "-1 1\n", "expected": "0\n"}]}
    def api(method):
        url = outputs["ApiUrl"]
        if method == "GET":
            url += "?" + urllib.parse.urlencode({"contest": contest, "request_id": "host-loss"})
        data = json.dumps(body).encode() if method == "POST" else b""
        req = AWSRequest(method=method, url=url, data=data, headers={"Content-Type": "application/json",
            "X-Amz-Content-Sha256": hashlib.sha256(data).hexdigest()})
        SigV4Auth(session.get_credentials().get_frozen_credentials(), "lambda", "ap-northeast-1").add_auth(req)
        with urllib.request.urlopen(urllib.request.Request(url, data=data if method == "POST" else None,
                headers=dict(req.headers.items()), method=method), timeout=35) as response:
            return json.load(response)
    accepted = api("POST")
    start = time.monotonic()
    replaced = False
    while time.monotonic() - start < 900:
        current = instances()
        if current and current.isdisjoint(original_instances) and not replaced:
            replaced = True
            print("ASG replaced the powered-off host", flush=True)
        view = api("GET")
        if view["status"] == "completed":
            assert view["verdict"] == "AC" and view["completed_cases"] == 3
            assert view["submission"] == accepted["submission"] and view["run"] == accepted["run"]
            break
        assert view["status"] != "infrastructure_error"
        time.sleep(3)
    else:
        raise TimeoutError("host-loss recovery exceeded deadline")
    assert replaced
    marker = json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=f"workers/faults/{accepted['run']}.json")["Body"].read())
    assert marker["event"] == "host_poweroff_before_result"
    assert api("POST") == accepted, "retry changed the acceptance identity"
    report["recovery_seconds"] = round(time.monotonic() - start, 3)
    report["checks"].update(host_poweroff_injected=True, different_ASG_instance_observed=True,
        same_submission_and_run_resumed=True, all_cases_AC_after_host_loss=True, retry_idempotent=True)
    sleep_pool(session, outputs, report)
    for key in ["ControlDLQ", "NodeDLQ"]:
        values = sqs.get_queue_attributes(QueueUrl=outputs[key], AttributeNames=["ApproximateNumberOfMessages"])["Attributes"]
        assert int(values["ApproximateNumberOfMessages"]) == 0
    report["checks"]["dead_letter_queues_empty"] = True
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: {len(report['checks'])} host-loss checks; report={args.report}", flush=True)


if __name__ == "__main__":
    main()
