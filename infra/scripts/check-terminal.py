#!/usr/bin/env python3
"""A valid late result must also survive losing the dispatch ACK checkpoint."""
import copy
import importlib.util
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import Mock, patch


class TerminalResults(unittest.TestCase):
    def operate(self, terminal, node_state, node="issued"):
        state = {"database": "closed-checkpoint", "pending": {}, "sent": [], "results": {}, "jobs": [["issued", {}]], "edges": [],
                 "view": {"status": terminal, "nodes": {"issued": node_state}}}
        original = copy.deepcopy(state)
        common = types.ModuleType("common")
        for name in ["BUCKET", "build_graph", "checkpoint_key", "encode", "load_json", "result_key", "s3", "sqs"]:
            setattr(common, name, Mock())
        common.load_json.return_value = (state, "etag")
        source = Path(__file__).resolve().parents[1] / "runtime/control.py"
        spec = importlib.util.spec_from_file_location("probe_control", source)
        module = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"common": common}):
            spec.loader.exec_module(module)
        message = {"kind": "result", "receipt": {"run": "pipeline"}, "node": node}
        if terminal in ["completed", "infrastructure_error"] and node == "issued" and node_state == "running":
            module.process(message)
            self.assertEqual(state, original)
            self.assertEqual(common.load_json.call_count, 1)
            common.s3.assert_not_called()
            self.assertEqual(common.s3.mock_calls, [])
            self.assertEqual(common.sqs.mock_calls, [])
        else:
            with self.assertRaisesRegex(ValueError, "not for a dispatched node"):
                module.process(message)

    def test_late_result_after_failure_without_dispatch_ack(self):
        self.operate("infrastructure_error", "running")

    def test_late_result_after_completion_without_dispatch_ack(self):
        self.operate("completed", "running")

    def test_unknown_node_is_rejected(self):
        self.operate("infrastructure_error", "running", "unknown")

    def test_never_issued_node_is_rejected(self):
        self.operate("infrastructure_error", "waiting")

    def test_active_run_requires_a_pending_request(self):
        self.operate("judging", "running")


if __name__ == "__main__":
    unittest.main()
