import json
import os
import urllib.parse

import boto3

from common import BUCKET, TABLE, checkpoint_key, ddb, encode, get_item, load_json, queue_control, result_key, s3


def handler(event, context):
    # DynamoDB Streams is the acceptance outbox. Only immutable receipt INSERTs.
    if event.get("Records", [{}])[0].get("eventSource") == "aws:dynamodb":
        for record in event["Records"]:
            if record["eventName"] != "INSERT":
                continue
            item = record["dynamodb"].get("NewImage", {})
            if not item.get("pk", {}).get("S", "").startswith("RECEIPT#"):
                continue
            receipt = json.loads(item["value"]["S"])
            if os.environ.get("CAPACITY_FUNCTION"):
                result = boto3.client("lambda").invoke(FunctionName=os.environ["CAPACITY_FUNCTION"],
                    Payload=b'{"action":"wake"}')
                result["Payload"].read()
                if result.get("FunctionError"):
                    raise RuntimeError("pool wake failed")
            queue_control({"kind": "submit", "contest": receipt["contest"], "receipt": receipt}, receipt["run"])
        return
    # Fargate infrastructure failure can happen before a worker publishes S3.
    if event.get("source") == "aws.batch":
        detail = event["detail"]
        job = get_item(f"JOB#{detail['jobId']}")
        if not job:
            raise RuntimeError("Batch job mapping not visible yet")
        receipt = job["receipt"]
        key = result_key(receipt, job["node"])
        try:
            s3.put_object(Bucket=BUCKET, Key=key, Body=encode({**{k: receipt[k] for k in ["contest", "submission", "run"]},
                "node": job["node"], "infrastructure_error": detail.get("statusReason", "Batch task failed")}).encode(), IfNoneMatch="*")
        except s3.exceptions.ClientError as exc:
            if exc.response["Error"]["Code"] != "PreconditionFailed":
                raise
        return
    for message in event["Records"]:
        body = json.loads(message["body"])
        for record in body.get("Records", []):
            if "s3" not in record:
                continue  # S3's notification configuration test message.
            key = urllib.parse.unquote_plus(record["s3"]["object"]["key"])
            result, _ = load_json(key)
            receipt = get_item(f"RUN#{result['run']}")
            if not receipt:
                raise RuntimeError("missing run mapping")
            queue_control({"kind": "result", "contest": result["contest"], "receipt": receipt,
                           "node": result["node"]}, key)
