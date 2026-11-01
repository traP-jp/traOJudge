"""One serialized controller owns warm/idle capacity transitions."""
import os
import time
import uuid

import boto3

from common import CONFIG, TABLE, ddb, get_item, put_item, sqs

scaling = boto3.client("autoscaling", config=CONFIG)


def reconcile(event):
    mode = get_item("POOL#mode") or {"warm": True, "activity": time.time()}
    action = event.get("action", "reconcile")
    if action == "wake":
        mode["activity"] = time.time()
        put_item("POOL#mode", mode)
        desired = 1
    else:
        pending = 0
        for variable in ["CONTROL_QUEUE", "NODE_QUEUE"]:
            values = sqs.get_queue_attributes(QueueUrl=os.environ[variable], AttributeNames=[
                "ApproximateNumberOfMessages", "ApproximateNumberOfMessagesNotVisible"])["Attributes"]
            pending += sum(int(v) for v in values.values())
        if action == "sleep":
            if pending:
                raise ValueError("cannot stop a pool with queued/in-flight work")
            mode = {"warm": False, "activity": 0}
            put_item("POOL#mode", mode)
        desired = 1 if pending or mode["warm"] or time.time() - mode["activity"] < 900 else 0
    scaling.set_desired_capacity(AutoScalingGroupName=os.environ["ASG"], DesiredCapacity=desired, HonorCooldown=False)
    return {"desired": desired, "warm": mode["warm"]}


def handler(event, context):
    # Account-wide reserved concurrency may be unavailable in a shared sandbox.
    # Serialize the capacity transition with a lease longer than Lambda timeout.
    owner = uuid.uuid4().hex
    for _ in range(100):
        try:
            ddb.put_item(TableName=TABLE, Item={"pk": {"S": "POOL#lease"}, "owner": {"S": owner},
                "expires": {"N": str(int(time.time()) + 90)}},
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
