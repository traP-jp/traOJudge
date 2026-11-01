"""IAM-only infrastructure acceptance/read API, independent of the product API."""
import base64
import datetime
import json
import time
import uuid
from urllib.parse import parse_qs

from botocore.exceptions import ClientError

from common import BUCKET, TABLE, checkpoint_key, ddb, digest, encode, get_item, identifier, load_json, s3


def response(status, body):
    return {"statusCode": status, "headers": {"content-type": "application/json", "cache-control": "no-store"}, "body": encode(body)}


def accept(body):
    contest = identifier(body["contest"])
    request_id = identifier(body["request_id"])
    key = f"RECEIPT#{contest}#{request_id}"
    body_hash = digest(encode(body))
    prior = get_item(key)
    if prior:
        if prior["request_hash"] != body_hash:
            raise ValueError("idempotency key reused with different content")
        return prior
    run = str(uuid.uuid5(uuid.NAMESPACE_URL, key))
    if body.get("rejudge_of"):
        original = get_item(f"RECEIPT#{contest}#{identifier(body['rejudge_of'])}")
        if not original:
            raise ValueError("unknown original submission")
        receipt = {**original, "request_id": request_id, "run": run, "request_hash": body_hash,
                   "rejudge_of": original["run"], "accepted_ns": time.time_ns()}
    else:
        if body.get("language", "c") != "c":
            raise ValueError("this infrastructure fixture supports native C17")
        source = body["source"]
        tests = body["tests"]
        if not isinstance(source, str) or len(source.encode()) > 65536 or not 1 <= len(tests) <= 32:
            raise ValueError("fixture limits: source <=64KiB, 1–32 tests")
        if any(not isinstance(t, dict) or set(t) != {"input", "expected"} or
               any(not isinstance(v, str) or len(v.encode()) > 65536 for v in t.values()) for t in tests):
            raise ValueError("each test needs input/expected strings <=64KiB")
        limits = body.get("limits", {"wall_seconds": 3, "cpu_seconds": 2, "memory_mb": 128})
        if set(limits) != {"wall_seconds", "cpu_seconds", "memory_mb"} or not (
            1 <= limits["wall_seconds"] <= 20 and 1 <= limits["cpu_seconds"] <= 15 and 32 <= limits["memory_mb"] <= 768
        ):
            raise ValueError("invalid fixture resource limits")
        artifacts = {}
        values = {"source": source}
        for i, case in enumerate(tests):
            values.update({f"test-{i}": case["input"], f"expected-{i}": case["expected"]})
        for name, value in values.items():
            object_key = f"inputs/{contest}/{run}/{name}"
            # Request retries can upload another version; the acceptance record pins one.
            obj = s3.put_object(Bucket=BUCKET, Key=object_key, Body=value.encode())
            artifacts[name] = {"key": object_key, "version": obj["VersionId"], "sha256": digest(value)}
        receipt = {"contest": contest, "request_id": request_id, "request_hash": body_hash,
                   "run": run, "tests": [{"id": i} for i in range(len(tests))], "limits": limits,
                   "artifacts": artifacts, "crash_after_checkpoint": bool(body.get("crash_after_checkpoint", False)),
                   "inject_host_loss": bool(body.get("inject_host_loss", False))}

    for _ in range(20):
        old = get_item(f"COUNTER#{contest}") or {"number": 0, "accepted_ns": 0}
        now = max(time.time_ns(), old["accepted_ns"] + 1)
        counter = {"number": old["number"] + 1, "accepted_ns": now}
        receipt["accepted_ns"] = now
        receipt["accepted_at"] = datetime.datetime.fromtimestamp(now / 1e9, datetime.timezone.utc).isoformat()
        if not body.get("rejudge_of"):
            receipt["submission"] = counter["number"]
        writes = [{"Put": {"TableName": TABLE, "Item": {"pk": {"S": key}, "value": {"S": encode(receipt)}},
                           "ConditionExpression": "attribute_not_exists(pk)"}}]
        if not body.get("rejudge_of"):
            condition = "attribute_not_exists(pk)" if old["number"] == 0 else "#v = :old"
            put = {"TableName": TABLE, "Item": {"pk": {"S": f"COUNTER#{contest}"}, "value": {"S": encode(counter)}},
                   "ConditionExpression": condition}
            if old["number"]:
                put.update(ExpressionAttributeNames={"#v": "value"}, ExpressionAttributeValues={":old": {"S": encode(old)}})
            writes.append({"Put": put})
        try:
            ddb.transact_write_items(TransactItems=writes)
            return receipt
        except ddb.exceptions.TransactionCanceledException:
            prior = get_item(key)
            if prior:
                if prior["request_hash"] != body_hash:
                    raise ValueError("idempotency key reused with different content")
                return prior
    raise RuntimeError("acceptance contention; retry with the same request_id")


def handler(event, context):
    try:
        method = event.get("requestContext", {}).get("http", {}).get("method", "POST")
        if method == "POST":
            data = event.get("body", "{}")
            if event.get("isBase64Encoded"):
                data = base64.b64decode(data).decode()
            receipt = accept(json.loads(data))
            return response(202, {k: receipt[k] for k in ["contest", "request_id", "submission", "run", "accepted_at"]})
        query = parse_qs(event.get("rawQueryString", ""))
        contest = identifier(query["contest"][0])
        request_id = identifier(query["request_id"][0])
        receipt = get_item(f"RECEIPT#{contest}#{request_id}")
        if not receipt:
            return response(404, {"error": "unknown request"})
        try:
            state, _ = load_json(checkpoint_key(receipt))
        except ClientError as exc:
            if exc.response["Error"]["Code"] in ["NoSuchKey", "404"]:
                return response(200, {"submission": receipt["submission"], "run": receipt["run"], "status": "queued", "completed_cases": 0, "total_cases": len(receipt["tests"])})
            raise
        return response(200, {"submission": receipt["submission"], "run": receipt["run"], **state["view"]})
    except (KeyError, ValueError, TypeError, json.JSONDecodeError) as exc:
        return response(400, {"error": str(exc)})
