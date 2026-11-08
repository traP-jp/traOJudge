"""Trusted node supervisor. Submitted programs run in credential-free, network-free containers.

One SQS delivery = one bipartite-DAG execution, possibly a multi-sandbox session.
Duplicate deliveries may repeat computation; an immutable canonical result wins once.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile
import threading
import time
import uuid

NAME = re.compile(r"[A-Za-z0-9_-]{1,64}\Z")


def validate_name(name):
    if not isinstance(name, str) or not NAME.fullmatch(name):
        raise ValueError("invalid artifact/process name")
    return name


def download(s3, reference, destination, bucket):
    if reference["bucket"] != bucket or not reference["key"].startswith(("inputs/", "runs/")):
        raise ValueError("object outside worker storage")
    if not reference["version_id"] or reference["version_id"] == "null":
        raise ValueError("immutable S3 version is required")
    if not 0 <= reference["byte_length"] <= 512 * 1024 * 1024:
        raise ValueError("input too large")
    response = s3.get_object(Bucket=bucket, Key=reference["key"], VersionId=reference["version_id"])
    digest = hashlib.sha256()
    count = 0
    try:
        with destination.open("xb") as output:
            for chunk in iter(lambda: response["Body"].read(1024 * 1024), b""):
                count += len(chunk)
                if count > reference["byte_length"]:
                    raise ValueError("input length exceeded")
                digest.update(chunk)
                output.write(chunk)
    finally:
        response["Body"].close()
    if count != reference["byte_length"] or digest.hexdigest() != reference["sha256"]:
        destination.unlink(missing_ok=True)
        raise ValueError("input SHA-256/length mismatch")
    destination.chmod(0o555 if reference.get("executable") else 0o444)


def upload(s3, bucket, key, path):
    digest = hashlib.sha256()
    count = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
            count += len(chunk)
    with path.open("rb") as source:
        result = s3.put_object(Bucket=bucket, Key=key, Body=source, ChecksumSHA256=base64.b64encode(digest.digest()).decode())
    return {"bucket": bucket, "key": key, "version_id": result["VersionId"], "sha256": digest.hexdigest(), "byte_length": count, "executable": bool(path.stat().st_mode & 0o111)}


def docker(*args, check=True):
    return subprocess.run(["docker", *args], check=check, text=True, capture_output=True)


def checked_file(path, maximum):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > maximum:
        raise ValueError("output must be a bounded regular file")
    return path


def execute(task, root, image):
    """Does not call AWS. Tests use exactly this supervisor and Docker isolation."""
    execution = task["execution"]
    if task["schema_version"] != 1 or task["image"] != image or not ("@sha256:" in image or image.startswith("sha256:")):
        raise ValueError("unsupported task/runtime image")
    validate_name(execution["name"])
    inputs = root / "inputs"
    roles = root / "roles"
    channels = root / "channels"
    roles.mkdir()
    channels.mkdir()
    channels.chmod(0o755)
    limits = execution["limits"]
    bindings = {}
    for item in execution["inputs"]:
        bindings.setdefault(item["artifact"], []).append(item.get("environment_variable"))
    main_script = next(i["artifact"] for i in execution["inputs"] if i.get("environment_variable") == "TRAOJUDGE_INIT_SCRIPT")
    definitions = [{"name": "main", "script": main_script, "inputs": execution.get("main_inputs") or list(bindings), "limits": limits}, *execution["peers"]]
    if len(definitions) > 9:
        raise ValueError("too many peer processes")
    names = [validate_name(d["name"]) for d in definitions]
    if len(set(names)) != len(names):
        raise ValueError("duplicate process names")
    for peer in execution["peers"]:
        folder = channels / validate_name(peer["name"])
        folder.mkdir(mode=0o755)
        folder.chmod(0o755)
        for direction in ("to-main", "from-main"):
            os.mkfifo(folder / direction, 0o666)
            (folder / direction).chmod(0o666)
    running = []
    timed_out = False
    try:
        for definition in definitions:
            role = definition["name"]
            folder = roles / role
            folder.mkdir()
            staged = folder / "inputs"
            staged.mkdir()
            staged.chmod(0o755)
            for artifact in definition["inputs"]:
                validate_name(artifact)
                if artifact not in bindings:
                    raise ValueError("process requested an undeclared input")
                shutil.copyfile(inputs / artifact, staged / artifact)
                (staged / artifact).chmod((inputs / artifact).stat().st_mode & 0o555)
            validate_name(definition["script"])
            if definition["script"] not in definition["inputs"]:
                raise ValueError("process script is not a mounted input")
            output = folder / "outputs"
            output.mkdir(mode=0o777)
            output.chmod(0o777)
            role_limits = definition["limits"]
            if not (1 <= role_limits["wall_seconds"] <= 300 and 32 <= role_limits["memory_mb"] <= 1024 and 0 < role_limits["cpus"] <= 2 and 16 <= role_limits["pids"] <= 256 and 1 <= role_limits["output_bytes"] <= 64 * 1024 * 1024):
                raise ValueError("limits exceed worker capacity")
            container = "judge-" + uuid.uuid4().hex
            args = ["docker", "create", "--name", container, "--network", "none", "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--user", "65534:65534", "--pids-limit", str(role_limits["pids"]), "--memory", f'{role_limits["memory_mb"]}m', "--memory-swap", f'{role_limits["memory_mb"]}m', "--cpus", str(role_limits["cpus"]), "--ulimit", f'fsize={role_limits["output_bytes"]}:{role_limits["output_bytes"]}', "--ulimit", "nofile=256:256", "--tmpfs", "/tmp:rw,nosuid,nodev,size=64m", "--workdir", "/work", "--mount", f'type=bind,src={staged},dst=/work/inputs,readonly', "--mount", f'type=bind,src={output},dst=/work/outputs', "--env", "PATH=/usr/local/bin:/usr/bin:/bin"]
            for artifact in definition["inputs"]:
                for variable in bindings[artifact]:
                    if variable:
                        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", variable):
                            raise ValueError("invalid environment variable")
                        args += ["--env", f"{variable}=/work/inputs/{artifact}"]
            if role == "main" and execution["peers"]:
                args += ["--mount", f'type=bind,src={channels},dst=/channels,readonly', "--env", "TRAOJUDGE_CHANNEL_DIR=/channels"]
            elif role != "main":
                args += ["--mount", f'type=bind,src={channels / role},dst=/channels,readonly', "--env", "TRAOJUDGE_CHANNEL_IN=/channels/from-main", "--env", "TRAOJUDGE_CHANNEL_OUT=/channels/to-main"]
            args += ["--entrypoint", "sh", image, f'/work/inputs/{definition["script"]}']
            subprocess.run(args, check=True, capture_output=True)
            stdout = (folder / "stdout").open("wb")
            stderr = (folder / "stderr").open("wb")
            try:
                process = subprocess.Popen(["docker", "start", "--attach", container], stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
            finally:
                stdout.close()
                stderr.close()
            running.append((container, role, process, time.monotonic() + role_limits["wall_seconds"]))
        while running[0][2].poll() is None:
            if any(p.poll() is None and time.monotonic() > deadline for _, _, p, deadline in running):
                timed_out = True
                for container, _, _, _ in running:
                    docker("kill", container, check=False)
                break
            # Bound combined writable files and captured logs, not just individual files.
            written = sum(p.stat().st_size for p in roles.rglob("*") if p.is_file() and "inputs" not in p.parts)
            if written > limits["output_bytes"]:
                raise ValueError("session output budget exceeded")
            time.sleep(0.05)
        # The trusted main process owns session completion. Stop leftover peers.
        for container, role, process, _ in running:
            if role != "main" and process.poll() is None:
                docker("kill", container, check=False)
        statuses = {}
        for container, role, process, _ in running:
            process.wait(timeout=15)
            state = json.loads(docker("inspect", "--format", "{{json .State}}", container).stdout)
            statuses[role] = {"exit_code": state["ExitCode"], "oom_killed": state["OOMKilled"], "timed_out": timed_out}
        main = statuses["main"]
        if timed_out or main["exit_code"] != 0:
            if execution.get("judge_errors") and not execution["outputs"]:
                verdict = "time_limit_exceeded" if timed_out else "memory_limit_exceeded" if main["oom_killed"] else "runtime_error"
                return {"verdict": verdict}, {}, statuses
            raise RuntimeError("session timeout" if timed_out else f'main exited with {main["exit_code"]}')
        stdout = checked_file(roles / "main" / "stdout", limits["output_bytes"])
        result = json.loads(stdout.read_text())
        if not isinstance(result, dict):
            raise ValueError("main stdout must be one JSON object")
        outputs = {validate_name(name): checked_file(roles / "main" / "outputs" / name, limits["output_bytes"]) for name in execution["outputs"]}
        return result, outputs, statuses
    finally:
        for container, _, process, _ in running:
            docker("rm", "-f", container, check=False)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def handle(task, s3, bucket, image, work):
    run_id, job_id = str(uuid.UUID(task["run_id"])), str(uuid.UUID(task["job_id"]))
    prefix = f"runs/{run_id}/{job_id}/{uuid.uuid4()}/"
    root = Path(tempfile.mkdtemp(prefix="node-", dir=work))
    report = {"schema_version": 1, "run_id": run_id, "job_id": job_id, "node": task["execution"]["name"], "outcome": "failed", "result": None, "outputs": {}, "logs": {}, "processes": {}, "error": None}
    try:
        (root / "inputs").mkdir()
        total = sum(r["byte_length"] for r in task["inputs"].values())
        if total > 1024 * 1024 * 1024:
            raise ValueError("combined input budget exceeds 1 GiB")
        for name, reference in task["inputs"].items():
            download(s3, reference, root / "inputs" / validate_name(name), bucket)
        result, outputs, statuses = execute(task, root, image)
        report.update(result=result, processes=statuses)
        report["outputs"] = {name: upload(s3, bucket, prefix + "artifacts/" + name, path) for name, path in outputs.items()}
        report["outcome"] = "completed"
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"[:2000]
        report["outputs"] = {}
    finally:
        try:
            for role in (root / "roles").glob("*"):
                for stream in ("stdout", "stderr"):
                    path = role / stream
                    if path.exists():
                        checked_file(path, 64 * 1024 * 1024)
                        report["logs"][role.name + "_" + stream] = upload(s3, bucket, prefix + f"logs/{role.name}/{stream}", path)
        finally:
            shutil.rmtree(root)
    return report


def main():
    import boto3
    from botocore.exceptions import ClientError
    bucket = os.environ["ARTIFACT_BUCKET"]
    image = os.environ["RUNTIME_IMAGE"]
    queue = os.environ["REQUEST_QUEUE_URL"]
    results = os.environ["RESULT_QUEUE_URL"]
    work = Path("/var/lib/traojudge")
    work.mkdir(parents=True, exist_ok=True)
    s3, sqs = boto3.client("s3"), boto3.client("sqs")
    docker("image", "inspect", image)
    while True:
        for message in sqs.receive_message(QueueUrl=queue, WaitTimeSeconds=20, MaxNumberOfMessages=1).get("Messages", []):
            receipt = message["ReceiptHandle"]
            stop = threading.Event()
            def heartbeat():
                while not stop.wait(30):
                    try:
                        sqs.change_message_visibility(QueueUrl=queue, ReceiptHandle=receipt, VisibilityTimeout=900)
                    except Exception as error:
                        print(json.dumps({"heartbeat_error": type(error).__name__}), flush=True)
            thread = threading.Thread(target=heartbeat, daemon=True)
            thread.start()
            try:
                task = json.loads(message["Body"])
                run_id, job_id = str(uuid.UUID(task["run_id"])), str(uuid.UUID(task["job_id"]))
                canonical_key = f"runs/{run_id}/{job_id}/result.json"
                try:
                    obj = s3.get_object(Bucket=bucket, Key=canonical_key)
                    try:
                        body = obj["Body"].read().decode()
                    finally:
                        obj["Body"].close()
                except ClientError as error:
                    if error.response["Error"]["Code"] not in ("NoSuchKey", "404"):
                        raise
                    body = json.dumps(handle(task, s3, bucket, image, work), separators=(",", ":"))
                    try:
                        s3.put_object(Bucket=bucket, Key=canonical_key, Body=body.encode(), IfNoneMatch="*")
                    except ClientError as error:
                        if error.response["ResponseMetadata"]["HTTPStatusCode"] not in (409, 412):
                            raise
                        obj = s3.get_object(Bucket=bucket, Key=canonical_key)
                        try:
                            body = obj["Body"].read().decode()
                        finally:
                            obj["Body"].close()
                sqs.send_message(QueueUrl=results, MessageBody=body)
                sqs.delete_message(QueueUrl=queue, ReceiptHandle=receipt)
                print(json.dumps({"run_id": run_id, "job_id": job_id, "outcome": json.loads(body)["outcome"]}), flush=True)
            except Exception as error:
                print(json.dumps({"error": type(error).__name__, "message": str(error)[:300]}), flush=True)
            finally:
                stop.set()
                thread.join(timeout=2)


if __name__ == "__main__":
    main()
