"""Single-message FIFO processing + conditional, atomic checkpoint/outbox publish."""
import base64
import json
import os
import subprocess
import tempfile

from botocore.exceptions import ClientError

from common import BUCKET, build_graph, checkpoint_key, encode, load_json, result_key, s3, sqs


def publish_pending(state, etag):
    changed = False
    for node, task in state["pending"].items():
        if node in state["sent"]:
            continue
        body = {"receipt": state["receipt"], "node": node, "task": task, "artifacts": state["artifacts"]}
        sqs.send_message(QueueUrl=os.environ["NODE_QUEUE"], MessageBody=encode(body))
        state["sent"].append(node)
        changed = True
    if changed:
        s3.put_object(Bucket=BUCKET, Key=checkpoint_key(state["receipt"]), Body=encode(state).encode(), IfMatch=etag)


def process(message):
    receipt = message["receipt"]
    key = checkpoint_key(receipt)
    try:
        state, etag = load_json(key)
    except ClientError as exc:
        if exc.response["Error"]["Code"] not in ["NoSuchKey", "404"]:
            raise
        state, etag = None, None
    if state is None:
        jobs, edges = build_graph(receipt)
        state = {"receipt": receipt, "jobs": jobs, "edges": edges, "database": None,
                 "artifacts": receipt["artifacts"], "pending": {}, "sent": [], "results": {}, "revision": 0,
                 "view": {"status": "queued", "completed_cases": 0, "total_cases": len(receipt["tests"]), "history": []}}
    if message["kind"] == "result" and message["node"] in state["results"]:
        # Replaying a completion also repairs any saved but unsent dispatches.
        publish_pending(state, etag)
        return
    if message["kind"] == "submit" and state["database"]:
        publish_pending(state, etag)
        return
    request = {"pipeline": receipt["run"], "jobs": state["jobs"], "edges": state["edges"]}
    terminal = False
    if message["kind"] == "result":
        node = message["node"]
        if node not in state["pending"]:
            # A sibling can still finish after an infrastructure failure ended
            # the run. It is a valid late result, not a poison queue message.
            issued_state = state["view"].get("nodes", {}).get(node)
            if state["view"]["status"] in ["completed", "infrastructure_error"] and issued_state in [
                "starting", "running", "completed", "failed", "cancelled"
            ]:
                return
            raise ValueError("completion is not for a dispatched node")
        result, _ = load_json(result_key(receipt, node))
        task = state["pending"].pop(node)
        if any(result[k] != receipt[k] for k in ["contest", "submission", "run"]) or result["node"] != node:
            raise ValueError("result identity mismatch")
        state["results"][node] = result
        state["artifacts"].update(result.get("artifacts", {}))
        request.update(completed=node, failed=bool(result.get("infrastructure_error")))
        if result.get("infrastructure_error"):
            state["view"]["status"] = "infrastructure_error"
            terminal = True
        elif task["phase"] == "compile" and result["exit_code"] != 0:
            request["failed"] = True
            state["view"].update(status="completed", verdict="CE")
            terminal = True
        elif task["phase"] == "check":
            verdict = result["node_output"]["verdict"]
            history = state["view"]["history"]
            history.append({"case": task["case"], "verdict": verdict, "completed_cases": len(history) + 1})
            state["view"]["completed_cases"] = len(history)
        elif task["phase"] == "summarize":
            summary = result["node_output"]
            priority = {"AC": 0, "WA": 1, "RE": 2, "MLE": 3, "OLE": 4, "TLE": 5}
            expected = [v["verdict"] for v in sorted(state["view"]["history"], key=lambda v: v["case"])]
            if (summary["completed_cases"] != len(receipt["tests"]) or summary["total_cases"] != len(expected)
                    or summary["case_verdicts"] != expected
                    or summary["verdict"] != max(expected, key=priority.__getitem__)):
                raise ValueError("summary does not match durable case results")
            state["view"]["summary"] = summary
    with tempfile.TemporaryDirectory() as temp:
        db = f"{temp}/workflow.sqlite"
        if state["database"]:
            with open(db, "wb") as stream:
                stream.write(base64.b64decode(state["database"]))
        request["database"] = db
        proc = subprocess.run([os.environ.get("ENGINE_PATH", "/opt/engine")], input=encode(request),
                              text=True, capture_output=True, timeout=20, check=True)
        result = json.loads(proc.stdout)
        with open(db, "rb") as stream:
            state["database"] = base64.b64encode(stream.read()).decode()
    for dispatch in result["dispatches"]:
        state["pending"][dispatch["id"]] = dispatch["task"]
    if terminal:
        state["pending"] = {}
    elif result["pipeline"] == "completed":
        priority = {"AC": 0, "WA": 1, "RE": 2, "MLE": 3, "OLE": 4, "TLE": 5}
        state["view"].update(status="completed", verdict=max(
            (v["verdict"] for v in state["view"]["history"]), key=lambda v: priority[v]))
    else:
        # Dispatch is not evidence of execution. A finished node is durable
        # evidence that this run has actually started.
        state["view"]["status"] = "judging" if state["results"] else "queued"
    state["revision"] += 1
    state["view"]["revision"] = state["revision"]
    state["view"]["nodes"] = result["states"]
    # One immutable S3 version contains the closed DB, progress and full outbox.
    # S3 conditional writes also fence an unexpectedly overlapping FIFO handler.
    params = {"IfMatch": etag} if etag else {"IfNoneMatch": "*"}
    saved = s3.put_object(Bucket=BUCKET, Key=key, Body=encode(state).encode(), **params)
    if receipt.get("crash_after_checkpoint") and state["revision"] == 1:
        raise RuntimeError("injected stop after durable checkpoint, before dispatch")
    publish_pending(state, saved["ETag"])


def handler(event, context):
    # batchSize=1; on error SQS must retain the message and block its FIFO group.
    for record in event["Records"]:
        process(json.loads(record["body"]))
