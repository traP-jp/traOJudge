import json
import os
import uuid

import boto3
from botocore.exceptions import ClientError

from common import BUCKET, CONFIG, put_item, result_key, s3

batch = boto3.client("batch", config=CONFIG)


def handler(event, context):
    for record in event["Records"]:
        message = json.loads(record["body"])
        receipt, node, task = message["receipt"], message["node"], message["task"]
        key = result_key(receipt, node)
        try:
            s3.head_object(Bucket=BUCKET, Key=key)
            continue
        except ClientError as exc:
            if exc.response["Error"]["Code"] not in ["404", "NoSuchKey", "NotFound"]:
                raise
        attempt = str(uuid.uuid4())
        inputs = {}
        for filename, artifact in task["inputs"].items():
            reference = message["artifacts"][artifact]
            inputs[filename] = {**reference, "url": s3.generate_presigned_url("get_object",
                Params={"Bucket": BUCKET, "Key": reference["key"], "VersionId": reference["version"]}, ExpiresIn=3600)}
        outputs = {}
        for name in task["outputs"]:
            object_key = f"artifacts/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/{node}/{attempt}/{name}"
            outputs[name] = {"key": object_key, "url": s3.generate_presigned_url("put_object",
                Params={"Bucket": BUCKET, "Key": object_key}, ExpiresIn=3600)}
        payload = {"task": task, "inputs": inputs, "outputs": outputs,
                   "identity": {**{k: receipt[k] for k in ["contest", "submission", "run"]}, "node": node},
                   "result_url": s3.generate_presigned_url("put_object", Params={"Bucket": BUCKET, "Key": key, "IfNoneMatch": "*"}, ExpiresIn=3600)}
        # A manifest avoids Batch's 8KiB container-overrides size limit.
        # Its URL is a credential: never log manifests, URLs or full overrides.
        payload_key = f"dispatch/{receipt['run']}/{node}/{attempt}.json"
        s3.put_object(Bucket=BUCKET, Key=payload_key, Body=json.dumps(payload).encode())
        manifest_url = s3.generate_presigned_url("get_object", Params={"Bucket": BUCKET, "Key": payload_key}, ExpiresIn=3600)
        # At-least-once SubmitJob may create duplicate tasks. Their artifact keys differ;
        # the conditional result object picks exactly one committed outcome.
        job = batch.submit_job(jobName=f"{os.environ['STACK_PREFIX']}-{node}", jobQueue=os.environ["JOB_QUEUE"],
            jobDefinition=os.environ["JOB_DEFINITION"], containerOverrides={"environment": [{"name": "MANIFEST_URL", "value": manifest_url}]})
        put_item(f"JOB#{job['jobId']}", {"receipt": receipt, "node": node})
        put_item(f"RUN#{receipt['run']}", receipt)
