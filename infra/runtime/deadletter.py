"""Quarantine exhausted node requests durably, then expose a terminal infrastructure error."""
import json
import time
from botocore.exceptions import ClientError
from common import BUCKET, encode, queue_control, result_key, s3


def handler(event, context):
    for record in event["Records"]:
        message = json.loads(record["body"])
        receipt, node = message["receipt"], message["node"]
        archived = {"origin": "node_dlq", "archived_at": time.time(), "request": message}
        s3.put_object(Bucket=BUCKET, Key=f"deadletters/{receipt['run']}/{node}.json", Body=encode(archived).encode())
        failure = {**{k: receipt[k] for k in ["contest", "submission", "run"]}, "node": node,
                   "infrastructure_error": "node_retry_exhausted"}
        key = result_key(receipt, node)
        try:
            s3.put_object(Bucket=BUCKET, Key=key, Body=encode(failure).encode(), IfNoneMatch="*")
        except ClientError as exc:
            if exc.response["Error"]["Code"] != "PreconditionFailed":
                raise
        queue_control({"kind": "result", "contest": receipt["contest"], "receipt": receipt, "node": node}, key)
    # Lambda's SQS mapping deletes the DLQ receipt only after archive + notification.
