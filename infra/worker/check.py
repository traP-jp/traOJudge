"""Trusted fixture validator; contestant tasks never receive expected answers."""
import json
from pathlib import Path

meta = json.loads(Path("inputs/meta").read_text())
if meta["timed_out"] or meta["exit_code"] in [-24, -9]:
    verdict = "TLE"
elif meta["exit_code"] == -25:
    verdict = "OLE"
elif meta["exit_code"] != 0:
    verdict = "RE"
else:
    verdict = "AC" if Path("inputs/actual").read_bytes().split() == Path("inputs/expected").read_bytes().split() else "WA"
print(json.dumps({"verdict": verdict}))
