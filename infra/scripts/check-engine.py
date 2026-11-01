#!/usr/bin/env python3
"""Restart the real Rust engine between each operation; move its checkpoint."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

parser = argparse.ArgumentParser()
parser.add_argument("engine", type=Path)
args = parser.parse_args()
engine = args.engine.resolve()
with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    ids = [str(uuid.uuid4()) for _ in range(4)]
    request = {"pipeline": str(uuid.uuid4()), "jobs": [[i, {"name": str(n)}] for n, i in enumerate(ids)],
               "edges": [[ids[0], ids[1]], [ids[0], ids[2]], [ids[1], ids[3]], [ids[2], ids[3]]]}

    def operate(**changes):
        # A fresh process and fresh local filename for every cloud-style restore.
        old, current = root / "saved", root / (uuid.uuid4().hex + ".sqlite")
        if old.exists():
            shutil.copyfile(old, current)
        body = {**request, **changes, "database": str(current)}
        proc = subprocess.run([engine], input=json.dumps(body), text=True, capture_output=True, check=True)
        shutil.copyfile(current, old)
        return json.loads(proc.stdout)

    state = operate()
    assert [d["id"] for d in state["dispatches"]] == [ids[0]]
    state = operate(completed=ids[0])
    assert {d["id"] for d in state["dispatches"]} == set(ids[1:3])
    assert not operate(completed=ids[0])["dispatches"], "duplicate result restarted a job"
    assert not operate(completed=ids[2])["dispatches"], "join started before both dependencies"
    state = operate(completed=ids[1])
    assert [d["id"] for d in state["dispatches"]] == [ids[3]]
    state = operate(completed=ids[3])
    assert state["pipeline"] == "completed" and set(state["states"].values()) == {"completed"}
    assert operate(completed=ids[2])["pipeline"] == "completed"
print("PASS: process restarts, checkpoint relocation, diamond join, duplicate completion, terminal monotonicity")
