"""Trusted final DAG node; consumes version-pinned case verdict artifacts."""
import json
from pathlib import Path

priority = {"AC": 0, "WA": 1, "RE": 2, "MLE": 3, "OLE": 4, "TLE": 5}
cases = sorted(Path("inputs").glob("case-*.json"), key=lambda p: int(p.stem.split("-")[1]))
if not cases or [int(p.stem.split("-")[1]) for p in cases] != list(range(len(cases))):
    raise ValueError("case verdict artifacts must be contiguous")
verdicts = [json.loads(p.read_text())["verdict"] for p in cases]
summary = {"verdict": max(verdicts, key=priority.__getitem__), "completed_cases": len(cases),
           "total_cases": len(cases), "case_verdicts": verdicts}
print(json.dumps(summary))
