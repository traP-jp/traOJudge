#!/usr/bin/env python3
"""Local recording adapter for real AWS responses; never schedules successors.

The browser receives a fixture-specific presentation of the acceptance/read API.
After cancellation, a read-only result observation shows the already dispatched
run finishing. It cannot alter the Engine, queue, policy, or verdict.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
import time
import uuid

from botocore.exceptions import ClientError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_scaling import Experiment
from workloads import TREE_DEMO_DELAYS_MS, TREE_DEMO_PARENTS as PARENTS, tree_demo


class Demo:
    def __init__(self, stack):
        self.aws = Experiment(stack, Path("work/tree-demo-unused.json"))
        self.contest = "tree-demo-" + uuid.uuid4().hex[:12]
        self.receipts = []
        self.trace = []
        self.started = None
        self.lock = threading.Lock()

    def submit(self):
        with self.lock:
            if self.receipts:
                return self.receipts
            self.started = time.time()
            requests = [tree_demo(self.contest, "non-ac", fail=True), tree_demo(self.contest, "all-ac")]
            with ThreadPoolExecutor(max_workers=2) as executor:
                self.receipts = list(executor.map(lambda data: self.aws.api("POST", data), requests))
            return self.receipts

    def view(self, receipt):
        value = self.aws.view(receipt)
        observed = {"at": time.time(), "request_id": receipt["request_id"], "api_view": value}
        # A cancelled Engine intentionally ignores late completions. Observe
        # the durable external result separately, without changing that state.
        existing_finished = False
        if value.get("pipeline_state") == "cancelled":
            node = str(uuid.uuid5(uuid.UUID(receipt["run"]), "run-4"))
            key = f"results/{receipt['contest']}/{receipt['submission']}/{receipt['run']}/{node}.json"
            try:
                result = self.aws.read(key)
                existing_finished = "execution" in result
            except ClientError as exc:
                if exc.response["Error"]["Code"] not in ["404", "NoSuchKey"]:
                    raise
        observed["already_dispatched_run4_result_saved"] = existing_finished
        with self.lock:
            self.trace.append(observed)
        history = {v["case"]: v["verdict"] for v in value.get("history", [])}
        nodes = value.get("nodes", {})
        def state(name):
            return nodes.get(str(uuid.uuid5(uuid.UUID(receipt["run"]), name)), "waiting")
        cancelled = value.get("pipeline_state") == "cancelled"
        cases = []
        for i, parent in enumerate(PARENTS):
            run, check = state(f"run-{i}"), state(f"check-{i}")
            if i in history:
                status, label = history[i], history[i]
            elif cancelled:
                if i == 4 and existing_finished:
                    status, label = "finished", "run終了／check未発行"
                elif run in ["starting", "running", "completed"] or check in ["starting", "running"]:
                    status, label = "issued", "発行済み・結果待ち"
                else:
                    status, label = "stopped", "未dispatch（停止）"
            elif run in ["starting", "running", "completed"]:
                status, label = "issued", "発行済み・結果待ち"
            else:
                status, label = "waiting", "親待ち" if parent is not None else "待機"
            cases.append({"id": i, "parent": parent, "status": status, "label": label})
        return {"request_id": receipt["request_id"], "submission": receipt["submission"],
            "pipeline": value.get("pipeline_state", "running"), "verdict": value.get("verdict"),
            "judged": value["completed_cases"], "total": value["total_cases"], "revision": value.get("revision", 0),
            "build_complete": state("compile") == "completed", "summary_complete": state("summarize") == "completed",
            "existing_finished": existing_finished, "cases": cases}

    def snapshot(self):
        if not self.receipts:
            return {"cards": [], "elapsed": 0}
        with ThreadPoolExecutor(max_workers=2) as executor:
            cards = list(executor.map(self.view, self.receipts))
        return {"cards": cards, "elapsed": time.time() - self.started}

    def evidence(self):
        return {"policy_profile": self.aws.outputs["PolicyProfile"],
            "policy_sha256": self.aws.outputs["PolicyFingerprint"], "receipts": self.receipts,
            "test_parents": PARENTS, "native_delays_ms": TREE_DEMO_DELAYS_MS,
            "raw_API_observations": self.trace, "started_at_unix": self.started}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    demo = Demo(args.stack)
    html = Path(__file__).with_name("index.html").read_bytes().replace(b"__TREE_PARENTS__", json.dumps(PARENTS).encode())
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def send(self, status, data, content_type="application/json"):
            encoded = data if isinstance(data, bytes) else json.dumps(data).encode()
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def do_GET(self):
            try:
                if self.path == "/": self.send(200, html, "text/html; charset=utf-8")
                elif self.path == "/state": self.send(200, demo.snapshot())
                elif self.path == "/evidence": self.send(200, demo.evidence())
                elif self.path == "/favicon.ico": self.send(204, b"")
                else: self.send(404, {"error": "not found"})
            except Exception as exc:
                self.send(502, {"error": type(exc).__name__})

        def do_POST(self):
            if self.path != "/submit" or self.headers.get("Content-Type") != "application/json":
                self.send(400, {"error": "invalid request"})
                return
            origin = self.headers.get("Origin")
            if origin and origin != f"http://{self.headers.get('Host')}":
                self.send(403, {"error": "invalid origin"})
                return
            try: self.send(202, demo.submit())
            except Exception as exc: self.send(502, {"error": type(exc).__name__})
    print("Local tree demo adapter ready", flush=True)
    ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
