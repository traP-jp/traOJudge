"""Trusted supervisor for one offline sandbox execution on a dedicated VM.

Only local, hash-pinned inputs are exposed to the workload. The host owns all
AWS transfers; this process has no online/presigned-URL fallback.
"""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


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
        execution_started_ns = time.time_ns()
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
        execution_finished_ns = time.time_ns()
    outcome = {**identity, "exit_code": proc.returncode, "timed_out": timed_out,
               "wall_seconds": round(time.monotonic() - started, 6), "artifacts": {}, "node_output": {},
               "execution": {"started_ns": execution_started_ns, "finished_ns": execution_finished_ns},
               "stderr_excerpt": stderr_path.read_bytes()[:4096].decode(errors="replace")}
    if task["phase"] == "run":
        (work / "outputs" / task["outputs"][1]).write_text(json.dumps({"exit_code": proc.returncode, "timed_out": timed_out}))
    elif task["phase"] in ["check", "summarize"]:
        if proc.returncode:
            outcome["infrastructure_error"] = "validator did not finish normally"
        else:
            outcome["node_output"] = json.loads(stdout_path.read_text())
            for output in task["outputs"]:
                (work / "outputs" / output).write_text(json.dumps(outcome["node_output"]))
    return outcome


def main():
    manifest = json.loads(Path("/work/manifest.json").read_text())
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
        data = (work / "inputs" / name).read_bytes()
        if len(data) > 16 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != reference["sha256"]:
            raise ValueError("input size/checksum mismatch")
        path = work / "inputs" / name
        path.write_bytes(data)
        path.chmod(0o555 if name == "binary" else 0o444)
        os.chown(path, 65532, 65532)
    outcome = execute(task, manifest["identity"])
    Path("/work/result.json").write_text(json.dumps(outcome))


if __name__ == "__main__":
    main()
