#!/usr/bin/env python3
"""The real Rust Engine owns non-AC cancellation; no adapter branch pruning."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import unittest
import uuid
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "infra/runtime"))


class Store:
    def __init__(self):
        self.values = {}
        self.version = 0

    def put_object(self, **request):
        key = request["Key"]
        if "IfMatch" in request:
            assert self.values[key][1] == request["IfMatch"]
        self.version += 1
        etag = str(self.version)
        self.values[key] = (json.loads(request["Body"]), etag)
        return {"ETag": etag, "VersionId": etag}

    def load(self, key):
        if key not in self.values:
            from botocore.exceptions import ClientError
            raise ClientError({"Error": {"Code": "NoSuchKey"}}, "GetObject")
        return copy.deepcopy(self.values[key])


class Tree(unittest.TestCase):
    def setUp(self):
        with patch("boto3.client", return_value=Mock()):
            import common
        self.common = common
        self.store, self.sent = Store(), []
        common.s3 = self.store
        common.load_json = self.store.load
        common.sqs = Mock()
        common.sqs.send_message.side_effect = lambda **r: self.sent.append(json.loads(r["MessageBody"]))
        spec = importlib.util.spec_from_file_location("tree_control", ROOT / "infra/runtime/control.py")
        self.control = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.control)
        self.receipt = {"contest": "tree", "submission": 1, "run": str(uuid.uuid4()),
            "tests": [{} for _ in range(5)], "test_parents": [None, 0, 1, None, 3], "artifacts": {},
            "limits": {"wall_seconds": 3, "cpu_seconds": 2, "memory_mb": 128}}
        self.environment = patch.dict(os.environ, {"ENGINE_PATH": str(ROOT / "target/debug/traojudge-infra-probe"),
            "NODE_QUEUE": "mocked-node-queue", "CAPACITY_FUNCTION": ""})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.control.process({"kind": "submit", "receipt": self.receipt})

    def finish(self, name, verdict="AC"):
        message = next(v for v in self.sent if v["task"]["name"] == name)
        node, task = message["node"], message["task"]
        result = {**{k: self.receipt[k] for k in ["contest", "submission", "run"]},
            "node": node, "exit_code": 0, "artifacts": {}, "node_output": {}}
        if task["phase"] == "check":
            result["node_output"] = {"verdict": verdict}
        elif task["phase"] == "summarize":
            result["node_output"] = {"verdict": "AC", "completed_cases": 5, "total_cases": 5,
                                     "case_verdicts": ["AC"] * 5}
        self.store.put_object(Key=self.common.result_key(self.receipt, node), Body=json.dumps(result))
        self.control.process({"kind": "result", "receipt": self.receipt, "node": node})

    def state(self):
        return self.store.load(self.common.checkpoint_key(self.receipt))[0]

    def test_non_AC_calls_engine_failure_and_never_releases_other_branch(self):
        self.finish("compile")
        self.assertEqual({v["task"]["name"] for v in self.sent}, {"compile", "run-0", "run-3"})
        self.finish("run-0")
        self.finish("check-0", "WA")
        stopped = self.state()
        nodes = {task["name"]: node for node, task in stopped["jobs"]}
        self.assertEqual(stopped["view"]["pipeline_state"], "cancelled")
        self.assertEqual(stopped["view"]["verdict"], "WA")
        self.assertEqual(stopped["view"]["nodes"][nodes["check-0"]], "failed")
        self.assertEqual(stopped["view"]["nodes"][nodes["run-3"]], "running")
        self.assertEqual(stopped["view"]["nodes"][nodes["check-3"]], "waiting")
        self.assertEqual(stopped["view"]["completed_cases"], 1)
        issued = copy.deepcopy(self.sent)
        # This root was already dispatched; its external execution may finish.
        # The scheduler must not dispatch even its immediate checker afterwards.
        self.finish("run-3")
        self.assertEqual(self.sent, issued)
        self.assertEqual(self.state(), stopped)
        self.assertEqual({v["task"]["name"] for v in self.sent}, {"compile", "run-0", "run-3", "check-0"})
        self.assertFalse(any(r.get("skipped") for r in stopped["results"].values()))

    def test_all_AC_releases_tree_and_summary_normally(self):
        completed = set()
        while True:
            next_job = next((v["task"]["name"] for v in self.sent if v["task"]["name"] not in completed), None)
            if next_job is None:
                break
            self.finish(next_job)
            completed.add(next_job)
        state = self.state()
        self.assertEqual(state["view"]["pipeline_state"], "completed")
        self.assertEqual(state["view"]["verdict"], "AC")
        self.assertEqual(state["view"]["completed_cases"], 5)
        self.assertEqual(len(completed), 12)
        self.assertTrue(all(s == "completed" for s in state["view"]["nodes"].values()))


if __name__ == "__main__":
    unittest.main()
