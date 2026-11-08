import base64
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import worker
from examples import SCENARIOS, Example

class Storage:
    def __init__(self):
        self.objects = {}
        self.counter = 0
    def put_object(self, Bucket, Key, Body, ChecksumSHA256=None):
        body = Body.read() if hasattr(Body, "read") else Body
        if ChecksumSHA256:
            assert base64.b64encode(hashlib.sha256(body).digest()).decode() == ChecksumSHA256
        self.counter += 1
        version = str(self.counter)
        self.objects[(Bucket, Key, version)] = body
        return {"VersionId": version}
    def get_object(self, Bucket, Key, VersionId):
        return {"Body": io.BytesIO(self.objects[(Bucket, Key, VersionId)])}
    def upload(self, name, body):
        result = self.put_object(Bucket="judge",Key="inputs/"+name,Body=body)
        return {"bucket":"judge","key":"inputs/"+name,"version_id":result["VersionId"],"sha256":hashlib.sha256(body).hexdigest(),"byte_length":len(body),"executable":False}

class DownloadTests(unittest.TestCase):
    def test_pins_version_and_rejects_corruption(self):
        storage = Storage()
        original = storage.upload("input", b"original")
        storage.upload("input", b"newest")
        with tempfile.TemporaryDirectory() as path:
            target = Path(path)/"file"
            worker.download(storage,original,target,"judge")
            self.assertEqual(target.read_bytes(),b"original")
            bad={**original,"sha256":"0"*64}
            with self.assertRaisesRegex(ValueError,"SHA-256"):
                worker.download(storage,bad,Path(path)/"bad","judge")
            self.assertFalse((Path(path)/"bad").exists())
    def test_rejects_foreign_storage_and_traversal_names(self):
        with tempfile.TemporaryDirectory() as path:
            with self.assertRaises(ValueError):
                worker.download(None,{"bucket":"other"},Path(path)/"bad","judge")
        for name in ["../escape","a/b","", "a"*65]:
            with self.assertRaises(ValueError): worker.validate_name(name)

@unittest.skipUnless(os.environ.get("JUDGE_TEST_IMAGE"),"set JUDGE_TEST_IMAGE to an immutable local Docker image ID")
class SandboxTests(unittest.TestCase):
    def run_definition(self, definition, storage):
        import uuid
        available = {a["name"]:a["reference"] for a in definition["artifacts"] if a["reference"]}
        reports=[]
        pending=list(definition["executions"])
        with tempfile.TemporaryDirectory(dir="/workspace/work") as directory:
            while pending:
                ready=next((e for e in pending if all(i["artifact"] in available for i in e["inputs"])),None)
                self.assertIsNotNone(ready,"definition cannot progress")
                task={"schema_version":1,"run_id":str(uuid.uuid4()),"job_id":str(uuid.uuid4()),"image":definition["image"],"execution":ready,"inputs":{i["artifact"]:available[i["artifact"]] for i in ready["inputs"]}}
                report=worker.handle(task,storage,"judge",definition["image"],directory)
                if report["outcome"] != "completed":
                    logs={name:storage.objects[(r["bucket"],r["key"],r["version_id"])].decode(errors="replace") for name,r in report["logs"].items()}
                    self.fail(json.dumps({"report":report,"logs":logs},indent=2))
                available.update(report["outputs"])
                reports.append(report)
                pending.remove(ready)
        return reports
    def test_all_pipeline_scenarios(self):
        image=os.environ["JUDGE_TEST_IMAGE"]
        for scenario, factory in SCENARIOS.items():
            with self.subTest(scenario=scenario):
                storage=Storage()
                reports=self.run_definition(factory(image,storage.upload),storage)
                result=reports[-1]["result"]
                if scenario in ["batch","interactive","run_twice"]: self.assertEqual(result["verdict"],"accepted")
                elif scenario=="heuristic": self.assertEqual(result,{"score":33.0,"trials":2})
                elif scenario=="coding_game": self.assertEqual(result,{"winner":"b","scores":{"a":4,"b":5}})
                elif scenario=="timeout": self.assertEqual(result["verdict"],"time_limit_exceeded")
    def test_sandbox_has_no_credentials_socket_or_external_network(self):
        image=os.environ["JUDGE_TEST_IMAGE"]
        storage=Storage();e=Example(image,storage.upload)
        e.node("isolation",'''python3 - <<'PY'
import os,socket,json
assert os.getuid()==65534
assert not os.path.exists('/var/run/docker.sock')
assert not any(k.startswith('AWS_') for k in os.environ)
try: open('/root/pwn','w');raise AssertionError('writable root')
except PermissionError: pass
s=socket.socket();s.settimeout(0.2)
try: s.connect(('169.254.169.254',80));raise AssertionError('reachable IMDS')
except OSError: pass
print(json.dumps({'isolated':True}))
PY
''')
        result=self.run_definition(e.finish(),storage)[0]["result"]
        self.assertTrue(result["isolated"])
    def test_missing_output_and_malformed_stdout_fail(self):
        import uuid
        for script, output in [('echo not-json',[]),('echo {}',['missing'])]:
            storage=Storage();e=Example(os.environ["JUDGE_TEST_IMAGE"],storage.upload)
            for name in output:e.output(name)
            e.node('broken',script,outputs=output)
            d=e.finish();execution=d['executions'][0]
            task={'schema_version':1,'run_id':str(uuid.uuid4()),'job_id':str(uuid.uuid4()),'image':d['image'],'execution':execution,'inputs':{a['name']:a['reference'] for a in d['artifacts'] if a['reference']}}
            with tempfile.TemporaryDirectory(dir='/workspace/work') as directory:
                report=worker.handle(task,storage,'judge',d['image'],directory)
            self.assertEqual(report['outcome'],'failed')
            self.assertFalse(report['outputs'])

if __name__=='__main__':unittest.main()
