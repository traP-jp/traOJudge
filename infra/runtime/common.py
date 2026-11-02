"""Infrastructure probe contracts; no contestant-facing application features."""
import hashlib
import json
import os
import re
import uuid

import boto3
from botocore.config import Config

CONFIG = Config(retries={"max_attempts": 4, "mode": "standard"}, connect_timeout=3, read_timeout=10)
REGION = os.environ.get("AWS_REGION", os.environ.get("AWS_DEFAULT_REGION", "ap-northeast-1"))
# A new bucket can redirect the global S3 endpoint. A SigV4 URL must be used
# against exactly the host that was signed, including for its very first GET.
s3 = boto3.client("s3", region_name=REGION, endpoint_url=f"https://s3.{REGION}.amazonaws.com",
                  config=CONFIG.merge(Config(signature_version="s3v4", s3={"addressing_style": "virtual"})))
sqs = boto3.client("sqs", config=CONFIG)
ddb = boto3.client("dynamodb", config=CONFIG)
BUCKET = os.environ.get("BUCKET", "")
TABLE = os.environ.get("TABLE", "")


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", value):
        raise ValueError("identifiers must contain 1–64 letters, digits, _ or -")
    return value


def get_item(key):
    item = ddb.get_item(TableName=TABLE, Key={"pk": {"S": key}}, ConsistentRead=True).get("Item")
    return json.loads(item["value"]["S"]) if item else None


def put_item(key, value):
    ddb.put_item(TableName=TABLE, Item={"pk": {"S": key}, "value": {"S": encode(value)}})


def load_json(key, version=None):
    params = {"Bucket": BUCKET, "Key": key}
    if version:
        params["VersionId"] = version
    obj = s3.get_object(**params)
    return json.loads(obj["Body"].read()), obj["ETag"]


def checkpoint_key(receipt):
    return f"checkpoints/{receipt['contest']}/{receipt['submission']}/{receipt['run']}.json"


def result_key(receipt, node):
    return f"results/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/{node}.json"


def queue_control(message, identity):
    sqs.send_message(QueueUrl=os.environ["CONTROL_QUEUE"], MessageBody=encode(message),
                     MessageGroupId=message["receipt"]["run"], MessageDeduplicationId=digest(identity))


def build_graph(receipt):
    """Compile once, fan out independent run/check branches, then join to summarize."""
    namespace = uuid.UUID(receipt["run"])
    nodes = []
    edges = []

    def node(name, phase, inputs, outputs, **extra):
        identity = str(uuid.uuid5(namespace, name))
        task = {"name": name, "phase": phase, "inputs": inputs, "outputs": outputs, **extra}
        nodes.append([identity, task])
        return identity

    compile_node = node("compile", "compile", {"source.c": "source"}, ["binary"],
                        command=["gcc", "-O2", "-std=c17", "inputs/source.c", "-o", "outputs/binary"],
                        wall_seconds=15, cpu_seconds=10, memory_mb=768)
    checks = []
    for i, case in enumerate(receipt["tests"]):
        run = node(f"run-{i}", "run", {"binary": "binary", "stdin": f"test-{i}"},
                   [f"actual-{i}", f"meta-{i}"], case=i, command=["inputs/binary"],
                   stdin="stdin", wall_seconds=receipt["limits"]["wall_seconds"],
                   cpu_seconds=receipt["limits"]["cpu_seconds"], memory_mb=receipt["limits"]["memory_mb"])
        check = node(f"check-{i}", "check", {"actual": f"actual-{i}", "meta": f"meta-{i}", "expected": f"expected-{i}"},
                     [f"verdict-{i}"], case=i, command=["python3", "/opt/check.py"],
                     wall_seconds=5, cpu_seconds=3, memory_mb=128)
        parent = receipt.get("test_parents", [None] * len(receipt["tests"]))[i]
        predecessor = compile_node if parent is None else str(uuid.uuid5(namespace, f"check-{parent}"))
        edges.extend([[predecessor, run], [run, check]])
        checks.append(check)
    summarize = node("summarize", "summarize", {f"case-{i}.json": f"verdict-{i}" for i in range(len(checks))},
                     ["summary"], command=["python3", "/opt/summarize.py"],
                     wall_seconds=5, cpu_seconds=3, memory_mb=128)
    edges.extend([[check, summarize] for check in checks])
    return nodes, edges


def notify_demand(receipt, state=None):
    """Repairable projection; S3 checkpoint is authoritative for run progress."""
    if not os.environ.get("CAPACITY_FUNCTION"):
        return
    import time
    from planning import project
    jobs, edges = (state["jobs"], state["edges"]) if state else build_graph(receipt)
    revision = state["revision"] if state else 0
    view = state["view"] if state else {}
    projection = project(jobs, edges, view.get("nodes"))
    if view.get("status") in ["completed", "infrastructure_error"]:
        projection.update(ready=0, potential_slots=0)
    projection.update(updated_at=time.time(), revision=revision, run=receipt["run"])
    try:
        ddb.put_item(TableName=TABLE, Item={"pk": {"S": "DEMAND#" + receipt["run"]},
            "value": {"S": encode(projection)}, "revision": {"N": str(revision)}},
            ConditionExpression="attribute_not_exists(revision) OR revision <= :new",
            ExpressionAttributeValues={":new": {"N": str(revision)}})
    except ddb.exceptions.ConditionalCheckFailedException:
        return
    boto3.client("lambda").invoke(FunctionName=os.environ["CAPACITY_FUNCTION"], InvocationType="Event",
                                  Payload=b'{"action":"reconcile"}')
