"""Small Lambda Runtime API client; container also runs unchanged outside AWS."""
import importlib
import json
import os
import traceback
import urllib.error
import urllib.request

endpoint = "http://" + os.environ["AWS_LAMBDA_RUNTIME_API"] + "/2018-06-01/runtime/invocation/"
handler = importlib.import_module(os.environ["MODE"]).handler
while True:
    response = urllib.request.urlopen(endpoint + "next")
    identity = response.headers["Lambda-Runtime-Aws-Request-Id"]
    event = json.load(response)
    try:
        output = handler(event, None)
        suffix, body = "/response", json.dumps(output).encode()
    except Exception as exc:
        traceback.print_exc()
        suffix = "/error"
        body = json.dumps({"errorType": type(exc).__name__, "errorMessage": str(exc), "stackTrace": traceback.format_exc().splitlines()}).encode()
    urllib.request.urlopen(urllib.request.Request(endpoint + identity + suffix, data=body, headers={"content-type": "application/json"})).close()
