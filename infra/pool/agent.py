"""Trusted EC2 host agent. Each node gets a fresh, offline gVisor sandbox."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time
import uuid
import urllib.request

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
polling = False
poll_started_at = None
active_admission = None
JOB_ROOT = Path("/var/lib/traojudge/jobs")
lock = threading.Lock()
POLICY = json.loads(os.environ["POOL_POLICY"])
if POLICY["limits"]["executions_per_instance"] != 1:
    raise ValueError("a VM executes exactly one node at a time")
RETIREMENT = Path("/var/lib/traojudge/retirement.json")
retirement = json.loads(RETIREMENT.read_text()) if RETIREMENT.exists() else {}
draining = bool(retirement)
drain_id = retirement.get("drain_id")
healthy = False
quarantined = False
unprotected = False
VISIBILITY = POLICY["queue"]["visibility_seconds"]
scaling = boto3.client("autoscaling", region_name=REGION, config=CONFIG)
def identify_host():
    global instance_id, asg, runtime_version
    token = urllib.request.urlopen(urllib.request.Request("http://169.254.169.254/latest/api/token", method="PUT",
        headers={"X-aws-ec2-metadata-token-ttl-seconds": "21600"}), timeout=3).read().decode()
    instance_id = urllib.request.urlopen(urllib.request.Request("http://169.254.169.254/latest/meta-data/instance-id",
        headers={"X-aws-ec2-metadata-token": token}), timeout=3).read().decode()
    asg = scaling.describe_auto_scaling_instances(InstanceIds=[instance_id])["AutoScalingInstances"][0]["AutoScalingGroupName"]
    runtime_version = subprocess.check_output(["/usr/local/bin/runsc", "--version"], text=True).splitlines()[0]


def poweroff_once(receipt, node, phase, **details):
    if not receipt.get("inject_host_loss") or receipt.get("host_loss_phase", "compile") != phase:
        return
    try:
        s3.put_object(Bucket=BUCKET, Key=f"workers/faults/{receipt['run']}.json", IfNoneMatch="*",
            Body=json.dumps({"event": "host_poweroff_before_result" if phase == "compile" else "host_poweroff_" + phase, "node": node, "worker": worker,
                             "at": time.time(), **details}).encode())
    except ClientError as exc:
        if exc.response["Error"]["Code"] != "PreconditionFailed":
            raise
    else:
        subprocess.run(["systemctl", "poweroff", "--no-block"], check=True)
        os._exit(77)


def send_result(message):
    r, node = message["receipt"], message["node"]
    key = f"results/{r['contest']}/{r['submission']}/{r['run']}/{node}.json"
    sqs.send_message(QueueUrl=CONTROL_QUEUE, MessageGroupId=r["run"],
        MessageDeduplicationId=hashlib.sha256(key.encode()).hexdigest(),
        MessageBody=json.dumps({"kind": "result", "contest": r["contest"], "receipt": r, "node": node}))


def reserve_poll():
    """Reserve one receive before drain can close admission or unprotect the VM."""
    global polling, poll_started_at
    with lock:
        if draining or quarantined or unprotected or not healthy:
            return False
        if polling or active:
            raise RuntimeError("one execution/poll reservation per VM invariant violated")
        polling = True
        poll_started_at = time.time()
        return True


def finish_poll(has_message):
    """Atomically hand a reserved receive to execution, including during drain."""
    global polling, poll_started_at, active, active_admission
    with lock:
        if not polling:
            raise RuntimeError("receive completed without an admission reservation")
        if has_message:
            if active or unprotected or quarantined:
                raise RuntimeError("reserved receive lost its protected execution capacity")
            active = 1
            active_admission = {"mode": "reserved_before_drain" if draining else "poll_reserved",
                                "poll_started_at": poll_started_at, "admitted_at": time.time()}
        else:
            active_admission = None
        polling = False
        poll_started_at = None
        return active_admission


def handle(record, admission):
    global active, active_admission, draining, quarantined
    with lock:
        if active != 1 or polling or unprotected or admission is not active_admission:
            raise RuntimeError("node execution requires its protected poll reservation")
    # Drain closes future admission. A receive already reserved before drain owns
    # one protected execution through durable delivery and verified cleanup.
    stop = threading.Event()
    def extend():
        while not stop.wait(POLICY["queue"]["renewal_seconds"]):
            sqs.change_message_visibility(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"], VisibilityTimeout=VISIBILITY)
    heartbeat = threading.Thread(target=extend, daemon=True)
    heartbeat.start()
    directory = None
    name = "node-" + uuid.uuid4().hex
    started = time.monotonic()
    try:
        message = json.loads(record["Body"])
        receipt, node, task = message["receipt"], message["node"], message["task"]
        receive_count = int(record.get("Attributes", {}).get("ApproximateReceiveCount", "1"))
        s3.put_object(Bucket=BUCKET, Key=f"workers/attempts/{receipt['run']}/{node}/{name}.json",
            Body=json.dumps({"worker": worker, "receive_count": receive_count, "phase": task["phase"], "at": time.time(), "admission": admission}).encode())
        if receipt.get("inject_poison") and task["phase"] == "compile":
            sqs.change_message_visibility(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"], VisibilityTimeout=1)
            return
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
        directory = JOB_ROOT / name
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
        if task["phase"] == "compile":
            poweroff_once(receipt, node, "compile")
        (directory / "manifest.json").write_text(json.dumps({"task": task, "identity": identity, "inputs": inputs}))
        command = ["docker", "run", "--name", name, "--label=traojudge.managed=true", "--pull=never", "--runtime=runsc", "--network=none",
            "--read-only", "--security-opt=no-new-privileges", "--cap-drop=ALL", "--cap-add=SETUID", "--cap-add=SETGID",
            "--cap-add=CHOWN", "--cap-add=KILL", "--cap-add=DAC_OVERRIDE", "--pids-limit=128", "--memory=1g", "--memory-swap=1g",
            "--cpus=1", "--tmpfs=/tmp:rw,noexec,nosuid,size=64m", "-v", str(directory) + ":/work",
            os.environ["IMAGE"], "--offline"]
        proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if task["phase"] == "run" and receipt.get("inject_host_loss") and receipt.get("host_loss_phase") == "run":
            time.sleep(2)
            running = subprocess.run(["docker", "inspect", "--format={{.State.Running}}", name], capture_output=True, text=True)
            if running.returncode == 0 and running.stdout.strip() == "true":
                poweroff_once(receipt, node, "run", docker_running=True)
        try:
            stdout, stderr = proc.communicate(timeout=POLICY["host"]["container_timeout_seconds"])
        except subprocess.TimeoutExpired:
            subprocess.run(["docker", "rm", "-f", name], capture_output=True)
            stdout, stderr = proc.communicate(timeout=10)
        if proc.returncode:
            print(json.dumps({"event": "container_error", "node": node, "stderr": stderr.decode(errors="replace")[-2000:]}), flush=True)
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
        outcome["admission"] = admission
        try:
            s3.put_object(Bucket=BUCKET, Key=key, Body=json.dumps(outcome).encode(), IfNoneMatch="*")
        except ClientError as exc:
            if exc.response["Error"]["Code"] != "PreconditionFailed":
                raise
        poweroff_once(receipt, node, "after_result", result_saved=True)
        send_result(message)
        sqs.delete_message(QueueUrl=NODE_QUEUE, ReceiptHandle=record["ReceiptHandle"])
        print(json.dumps({"event": "node_finished", "node": node, "seconds": outcome["container_seconds"]}), flush=True)
    except Exception as exc:
        print(json.dumps({"event": "node_error", "type": type(exc).__name__}), flush=True)
        # Retain SQS message. A lost host or send ACK resumes from the durable result.
    finally:
        stop.set()
        try:
            subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            remaining = subprocess.check_output(["docker", "ps", "-aq", "--filter=name=^/" + name + "$"]).strip()
            if remaining:
                raise RuntimeError("container cleanup failed")
            if directory:
                shutil.rmtree(directory)
                assert not directory.exists()
                s3.put_object(Bucket=BUCKET, Key=f"workers/cleanups/{receipt['run']}/{node}/{name}.json",
                    Body=json.dumps({"worker": worker, "container_absent": True, "workdir_absent": True, "at": time.time()}).encode())
        except Exception as exc:
            with lock:
                quarantined = draining = True
            print(json.dumps({"event": "cleanup_quarantine", "type": type(exc).__name__}), flush=True)
        finally:
            with lock:
                active -= 1
                active_admission = None


def publish_heartbeat():
    global draining, unprotected, drain_id, healthy
    try:
        command = json.loads(s3.get_object(Bucket=BUCKET, Key=f"workers/commands/{worker}.json")["Body"].read())
    except ClientError as exc:
        if exc.response["Error"]["Code"] not in ["NoSuchKey", "404"]:
            raise
        command = {}
    if command.get("action") == "drain":
        with lock:
            # Persist before unprotecting: agent restart must never resume a VM
            # an asynchronous ASG scale-in action may have selected already.
            draining = True
            healthy = False
            drain_id = command["drain_id"]
            temporary = RETIREMENT.with_suffix(".tmp")
            with temporary.open("w") as marker:
                json.dump({"drain_id": drain_id}, marker)
                marker.flush()
                os.fsync(marker.fileno())
            temporary.replace(RETIREMENT)
            directory_fd = os.open(RETIREMENT.parent, os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    with lock:
        count = active
        drain = draining
        pending_poll = polling
        poll_start = poll_started_at
        admission = active_admission
    if drain and count == 0 and not pending_poll and not quarantined and not unprotected:
        scaling.set_instance_protection(InstanceIds=[instance_id], AutoScalingGroupName=asg, ProtectedFromScaleIn=False)
        unprotected = True
    state = "quarantined" if quarantined else "drained" if unprotected else "draining" if drain else "ready"
    s3.put_object(Bucket=BUCKET, Key=f"workers/{worker}.json", Body=json.dumps({"state": state, "runtime": "runsc",
        "worker": worker, "instance_id": instance_id, "startup_cleanup": startup_cleanup,
        "runtime_version": runtime_version, "started_at": booted, "heartbeat_at": time.time(), "active": count,
        "polling": pending_poll, "poll_started_at": poll_start, "admission": admission,
        "drain_id": drain_id, "executions_per_instance": 1}).encode())
    with lock:
        healthy = not draining and not quarantined


def ready():
    global healthy, quarantined, draining
    while True:
        try:
            publish_heartbeat()
        except Exception as exc:
            # Losing the heartbeat/control channel must stop new execution. Never
            # acknowledge drain or clear quarantine after an uncertain cleanup.
            with lock:
                healthy = False
            print(json.dumps({"event": "heartbeat_error", "type": type(exc).__name__}), flush=True)
        time.sleep(POLICY["host"]["heartbeat_seconds"])


def poll_once():
    if not reserve_poll():
        return False
    try:
        records = sqs.receive_message(QueueUrl=NODE_QUEUE, WaitTimeSeconds=POLICY["host"]["poll_seconds"], MaxNumberOfMessages=1,
            MessageSystemAttributeNames=["ApproximateReceiveCount"]).get("Messages", [])
        if len(records) > 1:
            raise RuntimeError("SQS returned more messages than the single reserved execution")
    except Exception:
        finish_poll(False)
        raise
    admission = finish_poll(bool(records))
    if records:
        handle(records[0], admission)
    return True


def consume():
    while True:
        try:
            if not poll_once():
                time.sleep(1)
        except Exception as exc:
            print(json.dumps({"event": "poll_error", "type": type(exc).__name__}), flush=True)
            time.sleep(1)


def main():
    global startup_cleanup
    identify_host()
    JOB_ROOT.mkdir(parents=True, exist_ok=True, mode=0o700)
    # An agent restart is not a new VM: remove its predecessor's sandboxes before
    # claiming more work. Their unacknowledged SQS requests become visible again.
    orphans = subprocess.check_output(["docker", "ps", "-aq", "--filter=label=traojudge.managed=true"], text=True).split()
    if orphans:
        subprocess.run(["docker", "rm", "-f", *orphans], check=True, capture_output=True)
    old_directories = list(JOB_ROOT.iterdir())
    for directory in old_directories:
        shutil.rmtree(directory)
    startup_cleanup = {"orphan_containers_removed": len(orphans), "workdirs_removed": len(old_directories)}
    # A fresh ASG member is protected by its launch configuration. A normal agent
    # restart must prove protection before consuming; a retired boot stays retired.
    member = scaling.describe_auto_scaling_instances(InstanceIds=[instance_id])["AutoScalingInstances"][0]
    if not draining and (not member["ProtectedFromScaleIn"] or member["LifecycleState"] != "InService"):
        raise RuntimeError("host cannot claim work without InService scale-in protection")
    publish_heartbeat()
    threading.Thread(target=ready, daemon=True).start()
    # Sequential loop includes durable delivery and verified cleanup before reuse.
    consume()


if __name__ == "__main__":
    main()
