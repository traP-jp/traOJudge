"""One Fargate task per DAG node, without a task IAM role.

The root supervisor uses narrowly scoped presigned S3 operations. The workload
runs as another UID under seccomp + rlimits; each task is discarded afterwards.
"""
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import subprocess
import time
import urllib.error
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


opener = urllib.request.build_opener(NoRedirect)


def download(url):
    with opener.open(url, timeout=30) as response:
        return response.read(16 * 1024 * 1024 + 1)


def upload(url, data, conditional=False):
    headers = {"If-None-Match": "*"} if conditional else {}
    request = urllib.request.Request(url, data=data, method="PUT", headers=headers)
    with opener.open(request, timeout=30) as response:
        return response.headers.get("x-amz-version-id")


def execute(task, identity):
    work = Path("/work/node")
    cpu, memory = str(task["cpu_seconds"]), str(task["memory_mb"])
    command = ["/opt/sandbox", cpu, memory, *task["command"]]
    started = time.monotonic()
    stdin_path = work / "inputs" / task["stdin"] if task.get("stdin") else Path("/dev/null")
    stdout_path = work / "outputs" / (task["outputs"][0] if task["phase"] == "run" else "stdout")
    stderr_path = work / "stderr"
    timed_out = False
    with stdin_path.open("rb") as stdin, stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        proc = subprocess.Popen(command, cwd=work, stdin=stdin, stdout=stdout, stderr=stderr,
            start_new_session=True, env={"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": str(work), "LANG": "C.UTF-8", "TMPDIR": str(work / "outputs")})
        try:
            proc.wait(timeout=task["wall_seconds"])
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
        finally:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    outcome = {**identity, "exit_code": proc.returncode, "timed_out": timed_out,
               "wall_seconds": round(time.monotonic() - started, 6), "artifacts": {}, "node_output": {},
               "stderr_excerpt": stderr_path.read_bytes()[:4096].decode(errors="replace")}
    if task["phase"] == "run":
        (work / "outputs" / task["outputs"][1]).write_text(json.dumps({"exit_code": proc.returncode, "timed_out": timed_out}))
    elif task["phase"] == "check":
        if proc.returncode:
            outcome["infrastructure_error"] = "validator did not finish normally"
        else:
            outcome["node_output"] = json.loads(stdout_path.read_text())
    return outcome


def main():
    offline = "--offline" in sys.argv
    manifest = json.loads(Path("/work/manifest.json").read_text()) if offline else json.loads(download(os.environ["MANIFEST_URL"]))
    task = manifest["task"]
    work = Path("/work/node")
    work.mkdir(mode=0o700, exist_ok=True)
    os.chown(work, 65532, 65532)
    for directory in [work / "inputs", work / "outputs"]:
        directory.mkdir(mode=0o700, exist_ok=True)
        os.chown(directory, 65532, 65532)
    for name, reference in manifest["inputs"].items():
        if Path(name).name != name:
            raise ValueError("invalid input name")
        data = (work / "inputs" / name).read_bytes() if offline else download(reference["url"])
        if len(data) > 16 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != reference["sha256"]:
            raise ValueError("input size/checksum mismatch")
        path = work / "inputs" / name
        path.write_bytes(data)
        path.chmod(0o555 if name == "binary" else 0o444)
        os.chown(path, 65532, 65532)
    outcome = execute(task, manifest["identity"])
    if offline:
        Path("/work/result.json").write_text(json.dumps(outcome))
        return
    for name, reference in manifest["outputs"].items():
        path = work / "outputs" / name
        if not path.exists() and task["phase"] == "compile" and outcome["exit_code"]:
            continue
        if not path.is_file() or path.is_symlink() or path.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("invalid declared output")
        data = path.read_bytes()
        version = upload(reference["url"], data)
        outcome["artifacts"][name] = {"key": reference["key"], "version": version, "sha256": hashlib.sha256(data).hexdigest()}
    try:
        upload(manifest["result_url"], json.dumps(outcome).encode(), conditional=True)
    except urllib.error.HTTPError as exc:
        if exc.code != 412:
            raise
    print(json.dumps({"node": outcome["node"], "exit_code": outcome["exit_code"], "timed_out": outcome["timed_out"]}))


if __name__ == "__main__":
    main()
