#!/usr/bin/env python3
"""Real Rust checkpoint + fixture adapter: a failed parent prunes only its descendants."""
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
    def test_failed_parent_skips_descendants_but_keeps_other_root(self):
        with patch("boto3.client", return_value=Mock()):
            import common
        store, sent = Store(), []
        common.s3 = store
        common.load_json = store.load
        common.sqs = Mock()
        common.sqs.send_message.side_effect = lambda **r: sent.append(json.loads(r["MessageBody"]))
        spec = importlib.util.spec_from_file_location("tree_control", ROOT / "infra/runtime/control.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        receipt = {"contest": "tree", "submission": 1, "run": str(uuid.uuid4()),
                   "tests": [{} for _ in range(5)], "test_parents": [None, 0, 1, None, 3], "artifacts": {},
                   "limits": {"wall_seconds": 3, "cpu_seconds": 2, "memory_mb": 128}}
        executed = []
        with patch.dict(os.environ, {"ENGINE_PATH": str(ROOT / "target/debug/traojudge-infra-probe"),
                                     "NODE_QUEUE": "mocked-node-queue"}, clear=False):
            os.environ.pop("CAPACITY_FUNCTION", None)
            module.process({"kind": "submit", "receipt": receipt})
            while sent:
                msg = sent.pop(0)
                task, node = msg["task"], msg["node"]
                executed.append(task["name"])
                state, _ = store.load(common.checkpoint_key(receipt))
                result = {**{k: receipt[k] for k in ["contest", "submission", "run"]},
                          "node": node, "exit_code": 0, "artifacts": {}, "node_output": {}}
                if task["phase"] == "check":
                    result["node_output"] = {"verdict": "WA" if task["case"] == 0 else "AC"}
                elif task["phase"] == "summarize":
                    result["node_output"] = {"verdict": "WA", "completed_cases": 3, "total_cases": 5,
                        "skipped_cases": 2, "case_verdicts": ["WA", "SKIP", "SKIP", "AC", "AC"]}
                store.put_object(Key=common.result_key(receipt, node), Body=json.dumps(result))
                module.process({"kind": "result", "receipt": receipt, "node": node})
        state, _ = store.load(common.checkpoint_key(receipt))
        self.assertNotIn("run-1", executed)
        self.assertNotIn("run-2", executed)
        self.assertIn("run-4", executed)
        self.assertEqual(state["view"]["completed_cases"], 3)
        self.assertEqual(state["view"]["skipped_cases"], 2)
        self.assertEqual(state["view"]["verdict"], "WA")
        self.assertEqual(state["view"]["status"], "completed")
        self.assertTrue(all(s == "completed" for s in state["view"]["nodes"].values()))


if __name__ == "__main__":
    unittest.main()
