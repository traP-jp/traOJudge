"""Immutable acceptance receipts are the durable dispatch outbox."""
import json
import os
import boto3
from common import notify_demand, queue_control


def handler(event, context):
    for record in event["Records"]:
        if record["eventName"] != "INSERT":
            continue
        item = record["dynamodb"].get("NewImage", {})
        if not item.get("pk", {}).get("S", "").startswith("RECEIPT#"):
            continue
        receipt = json.loads(item["value"]["S"])
        notify_demand(receipt)
        result = boto3.client("lambda").invoke(FunctionName=os.environ["CAPACITY_FUNCTION"], Payload=b'{"action":"wake"}')
        result["Payload"].read()
        if result.get("FunctionError"):
            raise RuntimeError("pool wake failed")
        queue_control({"kind": "submit", "contest": receipt["contest"], "receipt": receipt}, receipt["run"])
