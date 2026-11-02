#!/usr/bin/env python3
"""Exercise both graph variants through the real Rust engine, not a scheduler mock."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import uuid
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("graph_common", ROOT / "infra/runtime/common.py")
common = importlib.util.module_from_spec(spec)
with patch("boto3.client", return_value=Mock()):
    spec.loader.exec_module(common)


class SequentialGraph(unittest.TestCase):
    def graph(self, flow):
        receipt = {"run": str(uuid.uuid4()), "tests": [{"id": i} for i in range(3)], "flow": flow,
                   "limits": {"wall_seconds": 3, "cpu_seconds": 2, "memory_mb": 128}}
        jobs, edges = common.build_graph(receipt)
        return receipt, jobs, edges

    def operate(self, db, receipt, jobs, edges, completed=None):
        request = {"database": str(db), "pipeline": receipt["run"], "jobs": jobs, "edges": edges}
        if completed is not None:
            request["completed"] = completed
        return json.loads(subprocess.check_output([str(ROOT / "target/debug/traojudge-infra-probe")],
                                                 input=json.dumps(request).encode()))

    def test_sequential_progress_and_summary_barrier(self):
        receipt, jobs, edges = self.graph("sequential")
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "checkpoint.sqlite"
            state = self.operate(db, receipt, jobs, edges)
            names = []
            while state["dispatches"]:
                self.assertEqual(len(state["dispatches"]), 1)
                dispatch = state["dispatches"][0]
                names.append(dispatch["task"]["name"])
                state = self.operate(db, receipt, jobs, edges, dispatch["id"])
            self.assertEqual(names, ["compile", "run-0", "check-0", "run-1", "check-1", "run-2", "check-2", "summarize"])
            self.assertEqual(state["pipeline"], "completed")
            self.assertTrue(all(v == "completed" for v in state["states"].values()))

    def test_existing_parallel_graph_is_preserved(self):
        receipt, jobs, edges = self.graph("parallel")
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "checkpoint.sqlite"
            state = self.operate(db, receipt, jobs, edges)
            state = self.operate(db, receipt, jobs, edges, state["dispatches"][0]["id"])
            self.assertEqual(sorted(v["task"]["name"] for v in state["dispatches"]), ["run-0", "run-1", "run-2"])

    def test_summary_aggregates_actual_case_artifacts(self):
        with tempfile.TemporaryDirectory() as temp:
            inputs = Path(temp) / "inputs"
            inputs.mkdir()
            for i, verdict in enumerate(["AC", "WA", "AC"]):
                (inputs / f"case-{i}.json").write_text(json.dumps({"verdict": verdict}))
            summary = json.loads(subprocess.check_output(["python3", str(ROOT / "infra/worker/summarize.py")], cwd=temp))
            self.assertEqual(summary, {"verdict": "WA", "completed_cases": 3, "total_cases": 3,
                                       "case_verdicts": ["AC", "WA", "AC"]})


if __name__ == "__main__":
    unittest.main()
