"""Real AWS pool readiness, latency and scale-to-zero checks."""
import concurrent.futures
import datetime
import json
import math
import statistics
import time
import uuid


def wait_ready(session, outputs, report):
    s3 = session.client("s3")
    start = time.monotonic()
    deadline = start + 600
    while time.monotonic() < deadline:
        objects = s3.list_objects_v2(Bucket=outputs["Bucket"], Prefix="workers/").get("Contents", [])
        for obj in objects:
            ready = json.loads(s3.get_object(Bucket=outputs["Bucket"], Key=obj["Key"])["Body"].read())
            if ready.get("runtime") == "runsc" and time.time() - ready.get("heartbeat_at", 0) < 30:
                report["pool"] = {"runtime": ready["runtime_version"], "slots": ready["slots"],
                                  "readiness_wait_seconds": round(time.monotonic() - start, 3)}
                group = session.client("autoscaling").describe_auto_scaling_groups(AutoScalingGroupNames=[outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
                instance = session.client("ec2").describe_instances(InstanceIds=[group["Instances"][0]["InstanceId"]])["Reservations"][0]["Instances"][0]
                report["pool"]["ec2_launch_to_ready_seconds"] = round(ready["started_at"] - instance["LaunchTime"].timestamp(), 3)
                print("gVisor pool ready", flush=True)
                return
        time.sleep(3)
    raise TimeoutError("EC2 pool did not become ready; inspect cloud-init/console output")


def performance(api, report, samples=10):
    contest = "perf-" + uuid.uuid4().hex[:12]
    source = '#include <stdio.h>\nint main(){long a,b;scanf("%ld%ld",&a,&b);printf("%ld",a+b);}'
    def sample(i):
        request = "sample-" + str(i)
        receipt = api("POST", {"contest": contest, "request_id": request, "source": source,
            "tests": [{"input": "1 2\n", "expected": "3\n"}, {"input": "4 5\n", "expected": "9\n"},
                      {"input": "-1 1\n", "expected": "0\n"}]})
        started = datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp()
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            view = api("GET", query={"contest": contest, "request_id": request})
            if view["status"] == "completed":
                assert view["verdict"] == "AC" and view["completed_cases"] == 3
                seconds = time.time() - started
                print(f"warm latency sample {i}: {seconds:.3f}s", flush=True)
                return round(seconds, 6)
            assert view["status"] != "infrastructure_error"
            time.sleep(0.1)
        raise TimeoutError("warm submission exceeded performance deadline")
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        values = list(pool.map(sample, range(samples)))
    values.sort()
    report["warm_latency"] = {"samples": samples, "concurrent_submissions": 2, "cases_per_submission": 3,
        "seconds": values, "median": round(statistics.median(values), 3),
        "p95": round(values[math.ceil(0.95 * len(values)) - 1], 3), "max": round(max(values), 3)}
    print(json.dumps(report["warm_latency"]), flush=True)
    assert report["warm_latency"]["p95"] <= 10, "warm P95 exceeds the 10-second target"
    report["checks"]["warm_p95_within_10_seconds"] = True


def sleep_pool(session, outputs, report):
    functions = session.client("lambda")
    scaling = session.client("autoscaling")
    deadline = time.monotonic() + 240
    while time.monotonic() < deadline:
        result = functions.invoke(FunctionName=outputs["CapacityFunction"], Payload=b'{"action":"sleep"}')
        value = json.loads(result["Payload"].read())
        if not result.get("FunctionError") and value.get("desired") == 0:
            break
        time.sleep(3)
    else:
        raise TimeoutError("pool did not accept the idle transition")
    while time.monotonic() < deadline:
        group = scaling.describe_auto_scaling_groups(AutoScalingGroupNames=[outputs["AutoScalingGroup"]])["AutoScalingGroups"][0]
        if group["DesiredCapacity"] == 0 and not group["Instances"]:
            report["checks"]["idle_pool_instances_zero"] = True
            report["idle_pool"] = {"desired": 0, "instances": 0, "minimum": group["MinSize"]}
            print("pool returned to zero EC2 instances", flush=True)
            return
        time.sleep(3)
    raise TimeoutError("EC2 instances did not terminate")
