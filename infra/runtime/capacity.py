"""AWS adapters for the separately testable, deployment-selected pool policy."""
import json
import os
import time
import uuid

import boto3
from common import BUCKET, CONFIG, TABLE, ddb, encode, get_item, put_item, s3, sqs
from policy import load_policy, retirement_plan, target_instances

POLICY = load_policy()
scaling = boto3.client("autoscaling", config=CONFIG)
cloudwatch = boto3.client("cloudwatch", config=CONFIG)


def workers(instances):
    current = {}
    for page in s3.get_paginator("list_objects_v2").paginate(Bucket=BUCKET, Prefix="workers/", Delimiter="/"):
        for obj in page.get("Contents", []):
            value = json.loads(s3.get_object(Bucket=BUCKET, Key=obj["Key"])["Body"].read())
            instance = value.get("instance_id")
            if instance not in instances or time.time() - value.get("heartbeat_at", 0) > POLICY["host"]["heartbeat_stale_seconds"]:
                continue
            if instance not in current or current[instance]["heartbeat_at"] < value["heartbeat_at"]:
                current[instance] = value
    return current


def demands():
    ready = forecast = 0
    params = {"TableName": TABLE, "ConsistentRead": True,
              "FilterExpression": "begins_with(pk, :prefix)", "ExpressionAttributeValues": {":prefix": {"S": "DEMAND#"}}}
    while True:
        response = ddb.scan(**params)
        for item in response["Items"]:
            value = json.loads(item["value"]["S"])
            if time.time() - value["updated_at"] < POLICY["scaling"]["demand_stale_seconds"]:
                ready += value["ready"]
                forecast += value["potential_slots"]
        if not response.get("LastEvaluatedKey"):
            return ready, forecast
        params["ExclusiveStartKey"] = response["LastEvaluatedKey"]


def command(worker, drain_id):
    s3.put_object(Bucket=BUCKET, Key=f"workers/commands/{worker['worker']}.json",
                  Body=encode({"action": "drain", "drain_id": drain_id, "issued_at": time.time()}).encode())


def reconcile(event):
    now = time.time()
    mode = get_item("POOL#mode") or {"sleeping": False, "activity": now, "retiring": {}}
    action = event.get("action", "reconcile")
    if action not in ["reconcile", "wake", "sleep"] or set(event) - {"action", "version", "id", "detail-type", "source", "account", "time", "region", "resources", "detail"}:
        raise ValueError("policy is deployment-selected; supported actions: reconcile, wake, sleep")
    queues = {}
    for name in ["CONTROL_QUEUE", "NODE_QUEUE"]:
        values = sqs.get_queue_attributes(QueueUrl=os.environ[name], AttributeNames=[
            "ApproximateNumberOfMessages", "ApproximateNumberOfMessagesNotVisible"])["Attributes"]
        queues[name] = {"visible": int(values["ApproximateNumberOfMessages"]),
                        "inflight": int(values["ApproximateNumberOfMessagesNotVisible"])}
    projected_ready, forecast = demands()
    ready = max(sum(queues["NODE_QUEUE"].values()), projected_ready)
    pending_control = sum(queues["CONTROL_QUEUE"].values())
    group = scaling.describe_auto_scaling_groups(AutoScalingGroupNames=[os.environ["ASG"]])["AutoScalingGroups"][0]
    desired = group["DesiredCapacity"]
    # Keep Pending:Wait/Pending:Proceed visible. Terminating members cannot accept work.
    instances = {v["InstanceId"]: v for v in group["Instances"] if v["LifecycleState"] == "InService" or v["LifecycleState"].startswith("Pending")}
    hosts = workers(instances)
    active = sum(w["active"] for w in hosts.values())
    if action == "wake":
        mode["activity"] = now
        # After explicit sleep, retain capacity for idle_seconds after this wake.
    if action == "sleep":
        if ready or forecast or pending_control or active:
            raise ValueError("cannot sleep with projected/queued/in-flight/active work")
        mode.update(sleeping=True, activity=0)
    target = target_instances(POLICY, max(ready, active), forecast, pending_control,
                              now, mode["activity"], mode["sleeping"])
    retiring = {i: token for i, token in mode["retiring"].items() if i in instances}
    current = max(desired, len(instances))
    if target < current:
        low_since = mode.setdefault("low_since", now)
    else:
        mode.pop("low_since", None)
    cooldown = target < current and action != "sleep" and now - mode["low_since"] < POLICY["scaling"]["scale_in_seconds"]
    # Previously issued retirements always finish; demand rebounds never resume an
    # unprotected VM that an older alarm action may already have selected.
    planned_target = current if cooldown else target
    safe_target, selected, transition = retirement_plan(planned_target, desired, instances, hosts, retiring, retiring)
    for instance in selected:
        retiring[instance] = uuid.uuid4().hex
    mode["retiring"] = retiring
    # Persist the irreversible decision before sending commands (retry repairs delivery).
    put_item("POOL#mode", mode)
    for instance, token in retiring.items():
        if instance in hosts:
            command(hosts[instance], token)
    if cooldown and not retiring:
        transition = "cooldown"
    if safe_target < 0 or safe_target > POLICY["limits"]["max_instances"]:
        raise RuntimeError("observed capacity exceeds declared bounds; refusing metric publication")
    # Missing publication never implies zero. CloudFormation owns all scale actions.
    cloudwatch.put_metric_data(Namespace=os.environ["METRIC_NAMESPACE"], MetricData=[{
        "MetricName": "SafeDesiredInstances", "Value": safe_target, "Unit": "Count"}])
    observation = {"at": now, "policy": POLICY["profile"], "strategy": POLICY["capacity"]["strategy"],
        "ready_executions": ready, "forecast_executions": forecast,
        "target_instances": target, "previous_desired": desired, "transition": transition,
        "published_safe_target": safe_target, "retiring_count": len(retiring),
        "queue": queues, "hosts": [{"worker": w["worker"], "state": w["state"], "active": w["active"],
            "polling": w.get("polling"), "poll_started_at": w.get("poll_started_at"), "admission": w.get("admission"),
            "protected": instances[i]["ProtectedFromScaleIn"]} for i, w in hosts.items()]}
    s3.put_object(Bucket=BUCKET, Key=f"workers/scaling/{now:.6f}-{uuid.uuid4().hex}.json", Body=encode(observation).encode())
    return {"desired": target, **observation}


def handler(event, context):
    owner = uuid.uuid4().hex
    for _ in range(100):
        try:
            ddb.put_item(TableName=TABLE, Item={"pk": {"S": "POOL#lease"}, "owner": {"S": owner},
                "expires": {"N": str(int(time.time()) + POLICY["scaling"]["lease_seconds"])}},
                ConditionExpression="attribute_not_exists(pk) OR #expires < :now",
                ExpressionAttributeNames={"#expires": "expires"}, ExpressionAttributeValues={":now": {"N": str(int(time.time()))}})
            break
        except ddb.exceptions.ConditionalCheckFailedException:
            time.sleep(0.05)
    else:
        raise RuntimeError("capacity transition busy")
    try:
        return reconcile(event)
    finally:
        ddb.delete_item(TableName=TABLE, Key={"pk": {"S": "POOL#lease"}}, ConditionExpression="#owner = :owner",
            ExpressionAttributeNames={"#owner": "owner"}, ExpressionAttributeValues={":owner": {"S": owner}})
