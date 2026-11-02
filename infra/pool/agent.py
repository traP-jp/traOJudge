"""Trusted EC2 host agent. Each node gets a fresh, offline gVisor sandbox."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time
import uuid

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

from artifacts import read_output

REGION = "ap-northeast-1"
BUCKET = os.environ["BUCKET"]
NODE_QUEUE = os.environ["NODE_QUEUE"]
CONTROL_QUEUE = os.environ["CONTROL_QUEUE"]
CONFIG = Config(connect_timeout=3, read_timeout=30, retries={"max_attempts": 4, "mode": "standard"})
s3 = boto3.client("s3", region_name=REGION, endpoint_url=f"https://s3.{REGION}.amazonaws.com", config=CONFIG)
sqs = boto3.client("sqs", region_name=REGION, config=CONFIG)
worker = uuid.uuid4().hex
booted = time.time()
active = 0
lock = threading.Lock()
runtime_version = subprocess.check_output(["/usr/local/bin/runsc", "--version"], text=True).splitlines()[0]


def send_result(message):
    r, node = message["receipt"], message["node"]
    key = f"results/{r['contest']}/{r['submission']}/{r['run']}/{node}.json"
    sqs.send_message(QueueUrl=CONTROL_QUEUE, MessageGroupId=r["run"],
        MessageDeduplicationId=hashlib.sha256(key.encode()).hexdigest(),
        MessageBody=json.dumps({"kind": "result", "contest": r["contest"], "receipt": r, "node": node}))


def handle(record):
    global active
    with lock:
        active += 1
    stop = threading.Event()
    def extend():
        while not stop.wait(30):
            sqs.change_message_visibility(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"], VisibilityTimeout=180)
    heartbeat = threading.Thread(target=extend, daemon=True)
    heartbeat.start()
    directory = None
    name = "node-" + uuid.uuid4().hex
    started = time.monotonic()
    try:
        message = json.loads(record["Body"])
        receipt, node, task = message["receipt"], message["node"], message["task"]
        key = f"results/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/{node}.json"
        try:
            s3.head_object(Bucket=BUCKET, Key=key)
        except ClientError as exc:
            if exc.response["Error"]["Code"] not in ["404", "NoSuchKey"]:
                raise
        else:
            send_result(message)
            sqs.delete_message(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"])
            return
        directory = Path("/var/lib/traojudge/jobs") / name
        (directory / "node/inputs").mkdir(parents=True, mode=0o755)
        (directory / "node/outputs").mkdir(mode=0o755)
        directory.chmod(0o755)
        inputs = {}
        for filename, artifact in task["inputs"].items():
            if Path(filename).name != filename:
                raise ValueError("invalid input name")
            ref = message["artifacts"][artifact]
            data = s3.get_object(Bucket=BUCKET, Key=ref["key"], VersionId=ref["version"])["Body"].read()
            if len(data) > 16 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != ref["sha256"]:
                raise ValueError("input checksum/size mismatch")
            (directory / "node/inputs" / filename).write_bytes(data)
            inputs[filename] = {"sha256": ref["sha256"]}
        identity = {**{k: receipt[k] for k in ["contest", "submission", "run"]}, "node": node}
        if receipt.get("inject_host_loss") and task["phase"] == "compile":
            try:
                s3.put_object(Bucket=BUCKET, Key=f"workers/faults/{receipt['run']}.json", IfNoneMatch="*",
                    Body=json.dumps({"event": "host_poweroff_before_result", "node": node, "at": time.time()}).encode())
            except ClientError as exc:
                if exc.response["Error"]["Code"] != "PreconditionFailed":
                    raise
            else:
                print(json.dumps({"event": "injected_host_poweroff", "node": node}), flush=True)
                subprocess.run(["systemctl", "poweroff", "--no-block"], check=True)
                os._exit(77)  # No result publication and no SQS delete/ACK.
        (directory / "manifest.json").write_text(json.dumps({"task": task, "identity": identity, "inputs": inputs}))
        command = ["docker", "run", "--name", name, "--pull=never", "--runtime=runsc", "--network=none",
            "--read-only", "--security-opt=no-new-privileges", "--cap-drop=ALL", "--cap-add=SETUID", "--cap-add=SETGID",
            "--cap-add=CHOWN", "--cap-add=KILL", "--cap-add=DAC_OVERRIDE", "--pids-limit=128", "--memory=1g", "--memory-swap=1g",
            "--cpus=1", "--tmpfs=/tmp:rw,noexec,nosuid,size=64m", "-v", str(directory) + ":/work",
            os.environ["IMAGE"], "--offline"]
        proc = subprocess.run(command, capture_output=True, timeout=90)
        if proc.returncode:
            print(json.dumps({"event": "container_error", "node": node, "stderr": proc.stderr.decode(errors="replace")[-2000:]}), flush=True)
            outcome = {**identity, "infrastructure_error": "gVisor node container failed"}
        else:
            outcome = json.loads((directory / "result.json").read_text())
            config = json.loads(subprocess.check_output(["docker", "inspect", "--format={{json .HostConfig}}", name]))
            assert config["Runtime"] == "runsc" and config["NetworkMode"] == "none" and config["ReadonlyRootfs"]
            outcome["isolation"] = {"runtime": config["Runtime"], "network": config["NetworkMode"], "read_only_rootfs": True}
            # Logical agent ID proves the intervals belong to the same host boot;
            # it is not an EC2 instance ID or any AWS account identifier.
            outcome["worker"] = worker
            for output in task["outputs"]:
                path = directory / "node/outputs" / output
                if not path.exists() and task["phase"] == "compile" and outcome["exit_code"]:
                    continue
                data = read_output(directory, path)
                object_key = f"artifacts/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/{node}/{name}/{output}"
                response = s3.put_object(Bucket=BUCKET, Key=object_key, Body=data)
                outcome["artifacts"][output] = {"key": object_key, "version": response["VersionId"], "sha256": hashlib.sha256(data).hexdigest()}
        outcome["container_seconds"] = round(time.monotonic() - started, 6)
        try:
            s3.put_object(Bucket=BUCKET, Key=key, Body=json.dumps(outcome).encode(), IfNoneMatch="*")
        except ClientError as exc:
            if exc.response["Error"]["Code"] != "PreconditionFailed":
                raise
        send_result(message)
        sqs.delete_message(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"])
        print(json.dumps({"event": "node_finished", "node": node, "seconds": outcome["container_seconds"]}), flush=True)
    except Exception as exc:
        print(json.dumps({"event": "node_error", "type": type(exc).__name__}), flush=True)
        # Retain SQS message. A lost host or send ACK resumes from the durable result.
    finally:
        stop.set()
        subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if directory:
            shutil.rmtree(directory, ignore_errors=True)
        with lock:
            active -= 1


def ready():
    while True:
        with lock:
            count = active
        s3.put_object(Bucket=BUCKET, Key=f"workers/{worker}.json", Body=json.dumps({"state": "ready", "runtime": "runsc",
            "runtime_version": runtime_version, "started_at": booted, "heartbeat_at": time.time(), "active": count, "slots": 2}).encode())
        time.sleep(10)


Path("/var/lib/traojudge/jobs").mkdir(parents=True, exist_ok=True, mode=0o700)
threading.Thread(target=ready, daemon=True).start()
def consume():
    while True:
        try:
            records = sqs.receive_message(QueueUrl=NODE_QUEUE, WaitTimeSeconds=20, MaxNumberOfMessages=1).get("Messages", [])
            for record in records:
                handle(record)
        except Exception as exc:
            print(json.dumps({"event": "poll_error", "type": type(exc).__name__}), flush=True)
            time.sleep(1)
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    list(pool.map(lambda _: consume(), range(2)))
