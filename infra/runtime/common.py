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
    """Native compile -> independent run/check branches, driven by the Rust engine."""
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
    for i, case in enumerate(receipt["tests"]):
        run = node(f"run-{i}", "run", {"binary": "binary", "stdin": f"test-{i}"},
                   [f"actual-{i}", f"meta-{i}"], case=i, command=["inputs/binary"],
                   stdin="stdin", wall_seconds=receipt["limits"]["wall_seconds"],
                   cpu_seconds=receipt["limits"]["cpu_seconds"], memory_mb=receipt["limits"]["memory_mb"])
        check = node(f"check-{i}", "check", {"actual": f"actual-{i}", "meta": f"meta-{i}", "expected": f"expected-{i}"},
                     [], case=i, command=["python3", "/opt/check.py"], wall_seconds=5, cpu_seconds=3, memory_mb=128)
        edges.extend([[compile_node, run], [run, check]])
    return nodes, edges
