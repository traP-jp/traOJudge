#!/usr/bin/env python3
"""Loopback-only demo bridge. AWS credentials and resource URLs stay server-side."""
import argparse
import datetime
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
import time
import urllib.parse
import urllib.request
import uuid

import boto3
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest

REGION = "ap-northeast-1"
SOURCE = '''#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <time.h>
int main(void) {
  long a, b;
  if (scanf("%ld%ld", &a, &b) != 2) return 1;
  long delay_ms = a == 1 ? 3400 : a == 4 ? 1000 : 2200;
  struct timespec delay = {delay_ms / 1000, (delay_ms % 1000) * 1000000};
  nanosleep(&delay, NULL);
  printf("%ld\\n", a + b);
  return 0;
}
'''
TESTS = [{"input": "1 2\n", "expected": "3\n"}, {"input": "4 5\n", "expected": "9\n"},
         {"input": "-1 1\n", "expected": "0\n"}]


def display(view):
    if view["status"] == "completed":
        return view["verdict"]
    if view["status"] == "infrastructure_error":
        return "IE"
    done = view["completed_cases"]
    return f"{done}/{view['total_cases']}" if done else "WJ"


class Demo:
    def __init__(self, stack):
        self.session = boto3.Session(region_name=REGION)
        self.outputs = {o["OutputKey"]: o["OutputValue"] for o in self.session.client("cloudformation").describe_stacks(
            StackName=stack)["Stacks"][0]["Outputs"]}
        self.s3 = self.session.client("s3")
        self.contest = "demo-" + uuid.uuid4().hex[:12]
        self.receipts, self.traces, self.audits = {}, {}, {}
        self.lock = threading.Lock()

    def api(self, method, body=None, query=None):
        url = self.outputs["ApiUrl"] + ("?" + urllib.parse.urlencode(query) if query else "")
        data = json.dumps(body).encode() if body is not None else b""
        request = AWSRequest(method=method, url=url, data=data, headers={"Content-Type": "application/json",
            "X-Amz-Content-Sha256": hashlib.sha256(data).hexdigest()})
        SigV4Auth(self.session.get_credentials().get_frozen_credentials(), "lambda", REGION).add_auth(request)
        with urllib.request.urlopen(urllib.request.Request(url, data=data if method == "POST" else None,
                headers=dict(request.headers.items()), method=method), timeout=35) as response:
            return json.load(response)

    def submit(self):
        request = "submission-" + uuid.uuid4().hex[:12]
        receipt = self.api("POST", {"contest": self.contest, "request_id": request,
            "source": SOURCE, "tests": TESTS, "limits": {"wall_seconds": 5, "cpu_seconds": 2, "memory_mb": 128}})
        with self.lock:
            self.receipts[request] = receipt
            self.traces[request] = []
        return receipt

    def status(self, request):
        receipt = self.receipts[request]
        view = self.api("GET", query={"contest": self.contest, "request_id": request})
        namespace = uuid.UUID(receipt["run"])
        nodes = [{"name": name, "state": view.get("nodes", {}).get(str(uuid.uuid5(namespace, name)), "waiting")}
                 for name in ["compile", "run-0", "check-0", "run-1", "check-1", "run-2", "check-2", "summarize"]]
        snapshot = {"observed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "elapsed_seconds": round(time.time() - datetime.datetime.fromisoformat(receipt["accepted_at"]).timestamp(), 3),
            "display": display(view), "view": view, "nodes": nodes}
        with self.lock:
            trace = self.traces[request]
            if not trace or trace[-1]["view"] != view:
                trace.append(snapshot)
        # The viewer needs domain progress, not raw engine UUID/state maps.
        public_view = {k: v for k, v in view.items() if k in [
            "status", "completed_cases", "total_cases", "revision", "verdict"]}
        return {**receipt, **snapshot, "view": public_view}

    def audit(self, request):
        if request in self.audits:
            return self.audits[request]
        receipt = self.receipts[request]
        key = f"checkpoints/{self.contest}/{receipt['submission']}/{receipt['run']}.json"
        state = json.loads(self.s3.get_object(Bucket=self.outputs["Bucket"], Key=key)["Body"].read())
        assert state["view"]["status"] == "completed" and state["view"]["verdict"] == "AC"
        assert state["view"]["summary"]["case_verdicts"] == ["AC", "AC", "AC"]
        assert len(state["results"]) == 8
        assert all(r["isolation"] == {"runtime": "runsc", "network": "none", "read_only_rootfs": True}
                   for r in state["results"].values())
        binary = state["artifacts"]["binary"]
        compile_id = str(uuid.uuid5(uuid.UUID(receipt["run"]), "compile"))
        assert state["results"][compile_id]["artifacts"]["binary"] == binary
        assert all(t["inputs"]["binary"] == "binary" for _, t in state["jobs"] if t["phase"] == "run")
        runs = [{"case": t["case"], "worker": state["results"][node]["worker"],
                 **state["results"][node]["execution"]} for node, t in state["jobs"] if t["phase"] == "run"]
        overlaps = []
        for i, left in enumerate(runs):
            for right in runs[i + 1:]:
                overlap = min(left["finished_ns"], right["finished_ns"]) - max(left["started_ns"], right["started_ns"])
                if left["worker"] == right["worker"] and overlap > 0:
                    overlaps.append({"cases": [left["case"], right["case"]], "seconds": round(overlap / 1e9, 6)})
        assert overlaps, "no same-host run execution overlap observed"
        origin = min(r["started_ns"] for r in runs)
        timeline = [{"case": r["case"], "start_seconds": round((r["started_ns"] - origin) / 1e9, 6),
                     "end_seconds": round((r["finished_ns"] - origin) / 1e9, 6)} for r in runs]
        summary_start = state["results"][str(uuid.uuid5(uuid.UUID(receipt["run"]), "summarize"))]["execution"]["started_ns"]
        assert all(state["results"][node]["execution"]["finished_ns"] <= summary_start
                   for node, t in state["jobs"] if t["phase"] == "check")
        summary_id = str(uuid.uuid5(uuid.UUID(receipt["run"]), "summarize"))
        task = dict(state["jobs"])[summary_id]
        for artifact in task["inputs"].values():
            ref = state["artifacts"][artifact]
            data = self.s3.get_object(Bucket=self.outputs["Bucket"], Key=ref["key"], VersionId=ref["version"])["Body"].read()
            assert hashlib.sha256(data).hexdigest() == ref["sha256"]
            assert json.loads(data)["verdict"] == "AC"
        result = {"completed_nodes": len(state["results"]), "binary_sha256": binary["sha256"],
            "run_execution_timeline": timeline, "same_host_overlaps": overlaps,
            "case_completion_order": [v["case"] for v in state["view"]["history"]],
            "binary_version": binary["version"], "summary": state["view"]["summary"],
            "summary_sha256": state["artifacts"]["summary"]["sha256"],
            "checks": {"all_8_nodes_completed": True, "all_nodes_offline_gvisor": True,
                "same_versioned_binary_binding_for_all_runs": True, "summary_reads_verified_case_artifacts": True,
                "same_host_run_execution_overlaps": True, "summarize_starts_after_all_checks": True}}
        self.audits[request] = result
        return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stack", required=True)
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    demo = Demo(args.stack)
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def send(self, value, status=200):
            data = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            url = urllib.parse.urlparse(self.path)
            if url.path == "/":
                data = Path(__file__).with_name("index.html").read_bytes()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            try:
                request = urllib.parse.parse_qs(url.query).get("id", [""])[0]
                if url.path == "/status":
                    self.send(demo.status(request))
                elif url.path == "/audit":
                    self.send(demo.audit(request))
                elif url.path == "/evidence":
                    with demo.lock:
                        self.send({"receipts": list(demo.receipts.values()), "traces": demo.traces, "audits": demo.audits})
                else:
                    self.send({"error": "unknown route"}, 404)
            except Exception as exc:
                # Never return exception URLs or credential-bearing SDK payloads.
                self.send({"error": type(exc).__name__}, 500)

        def do_POST(self):
            if self.path != "/submit" or self.headers.get("Origin") != f"http://127.0.0.1:{args.port}":
                self.send({"error": "same-origin submit required"}, 403)
                return
            try:
                self.send(demo.submit(), 202)
            except Exception as exc:
                self.send({"error": type(exc).__name__}, 500)
    print(f"Demo ready at http://127.0.0.1:{args.port}", flush=True)
    ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
