#!/usr/bin/env python3
"""Exercise fan-out and the summary join through the real Rust engine."""
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


class ParallelGraph(unittest.TestCase):
    def graph(self):
        receipt = {"run": str(uuid.uuid4()), "tests": [{"id": i} for i in range(3)],
                   "limits": {"wall_seconds": 3, "cpu_seconds": 2, "memory_mb": 128}}
        jobs, edges = common.build_graph(receipt)
        return receipt, jobs, edges

    def operate(self, db, receipt, jobs, edges, completed=None):
        request = {"database": str(db), "pipeline": receipt["run"], "jobs": jobs, "edges": edges}
        if completed is not None:
            request["completed"] = completed
        return json.loads(subprocess.check_output([str(ROOT / "target/debug/traojudge-infra-probe")],
                                                 input=json.dumps(request).encode()))

    def test_out_of_order_cases_and_summary_barrier(self):
        receipt, jobs, edges = self.graph()
        by_name = {t["name"]: node for node, t in jobs}
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "checkpoint.sqlite"
            state = self.operate(db, receipt, jobs, edges)
            self.assertEqual([v["task"]["name"] for v in state["dispatches"]], ["compile"])
            state = self.operate(db, receipt, jobs, edges, by_name["compile"])
            self.assertEqual(sorted(v["task"]["name"] for v in state["dispatches"]), ["run-0", "run-1", "run-2"])
            for i in [1, 2, 0]:
                state = self.operate(db, receipt, jobs, edges, by_name[f"run-{i}"])
                self.assertEqual([v["task"]["name"] for v in state["dispatches"]], [f"check-{i}"])
                state = self.operate(db, receipt, jobs, edges, by_name[f"check-{i}"])
                self.assertEqual([v["task"]["name"] for v in state["dispatches"]], ["summarize"] if i == 0 else [])
                self.assertNotEqual(state["pipeline"], "completed")
            state = self.operate(db, receipt, jobs, edges, by_name["summarize"])
            self.assertEqual(state["pipeline"], "completed")
            self.assertTrue(all(v == "completed" for v in state["states"].values()))

    def test_duplicate_case_does_not_release_summary_early(self):
        receipt, jobs, edges = self.graph()
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "checkpoint.sqlite"
            state = self.operate(db, receipt, jobs, edges)
            state = self.operate(db, receipt, jobs, edges, state["dispatches"][0]["id"])
            self.assertEqual(sorted(v["task"]["name"] for v in state["dispatches"]), ["run-0", "run-1", "run-2"])
            run = next(v["id"] for v in state["dispatches"] if v["task"]["name"] == "run-1")
            state = self.operate(db, receipt, jobs, edges, run)
            check = state["dispatches"][0]["id"]
            self.operate(db, receipt, jobs, edges, check)
            state = self.operate(db, receipt, jobs, edges, check)
            self.assertEqual(state["dispatches"], [])
            self.assertNotEqual(state["pipeline"], "completed")

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
