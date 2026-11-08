#!/usr/bin/env python3
"""AWS acceptance test. Raw account/resource identifiers stay in memory only."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import sys
import time
import uuid

import boto3
from botocore.exceptions import ClientError
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/"deploy/judge"))
from examples import SCENARIOS


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--stack",required=True)
    parser.add_argument("--report",type=Path,required=True)
    args=parser.parse_args()
    cf,ssm,s3,sqs=[boto3.client(service,region_name="ap-northeast-1") for service in ["cloudformation","ssm","s3","sqs"]]
    outputs={o["OutputKey"]:o["OutputValue"] for o in cf.describe_stacks(StackName=args.stack)["Stacks"][0]["Outputs"]}
    controller=outputs["ControllerInstanceId"]
    bucket=outputs["ArtifactBucket"]
    def remote(code,timeout=120):
        response=ssm.send_command(InstanceIds=[controller],DocumentName="AWS-RunShellScript",Parameters={"commands":["python3 - <<'JUDGE_PYTHON'\n"+code+"\nJUDGE_PYTHON"],"executionTimeout":[str(timeout)]})
        command=response["Command"]["CommandId"]
        end=time.monotonic()+timeout+30
        while time.monotonic()<end:
            try:
                result=ssm.get_command_invocation(CommandId=command,InstanceId=controller)
            except ClientError as error:
                if error.response["Error"]["Code"]=="InvocationDoesNotExist":
                    time.sleep(1);continue
                raise
            if result["Status"]=="Success": return json.loads(result["StandardOutputContent"])
            if result["Status"] in ["Failed","Cancelled","TimedOut"]:
                raise RuntimeError("remote verification failed ("+result["Status"]+"): "+result["StandardErrorContent"][-3000:]+result["StandardOutputContent"][-1000:])
            time.sleep(1)
        raise TimeoutError("SSM command did not finish")
    ready=False
    for attempt in range(90):
        try:
            info=remote("import json,urllib.request\nfrom pathlib import Path\nr=urllib.request.urlopen('http://127.0.0.1:8080/health',timeout=2)\nprint(json.dumps({'health':r.read().decode(),'image':Path('/etc/traojudge/runtime-image').read_text().strip()}))",20)
            ready=True;break
        except ClientError as error:
            if error.response["Error"]["Code"] in ["AccessDenied","AccessDeniedException","UnauthorizedOperation"]:raise
            if error.response["Error"]["Code"] not in ["InvalidInstanceId"]:raise
        except RuntimeError: pass
        time.sleep(5)
    if not ready:raise RuntimeError("controller did not become healthy")
    print("Controller is healthy; checking the internal ALB", flush=True)
    elb=boto3.client("elbv2",region_name="ap-northeast-1")
    deadline=time.monotonic()+180
    while True:
        health=elb.describe_target_health(TargetGroupArn=outputs["ControllerTargetGroupArn"])["TargetHealthDescriptions"]
        if health and all(item["TargetHealth"]["State"]=="healthy" for item in health):break
        if time.monotonic()>deadline:
            raise RuntimeError("ALB target failed health check: "+json.dumps([h["TargetHealth"] for h in health]))
        time.sleep(3)
    remote("import json,urllib.request,time,sys\nurl="+repr(outputs["InternalApiUrl"]+"/health")+"\nfor n in range(30):\n try:\n  assert urllib.request.urlopen(url,timeout=3).read()==b'ok';break\n except Exception as error:\n  print(type(error).__name__+': '+str(error),file=sys.stderr,flush=True)\n  if n==29:raise\n  time.sleep(2)\nprint(json.dumps({'internal_api_healthy':True}))",240)
    print("Internal ALB is healthy; submitting seven scenarios", flush=True)
    image=info["image"]
    documents={}
    originals=[]
    def upload(name,body):
        key=f"inputs/{uuid.uuid4()}/{name}"
        digest=hashlib.sha256(body).digest()
        result=s3.put_object(Bucket=bucket,Key=key,Body=body,ChecksumSHA256=base64.b64encode(digest).decode())
        reference={"bucket":bucket,"key":key,"version_id":result["VersionId"],"sha256":digest.hex(),"byte_length":len(body),"executable":False}
        originals.append(reference)
        return reference
    for scenario,factory in SCENARIOS.items():
        documents[scenario]={"run_id":str(uuid.uuid4()),"definition":factory(image,upload)}
    # Replace one key after pinning its version. The batch must still use its original bytes.
    original=next(a["reference"] for a in documents["batch"]["definition"]["artifacts"] if a["name"]=="case_input")
    replacement=b"0 0\n"
    s3.put_object(Bucket=bucket,Key=original["key"],Body=replacement,ChecksumSHA256=base64.b64encode(hashlib.sha256(replacement).digest()).decode())
    encoded=base64.b64encode(json.dumps(documents).encode()).decode()
    remote(f'''import base64,json,urllib.request
requests=json.loads(base64.b64decode({encoded!r}))
for item in requests.values():
 request=urllib.request.Request('http://127.0.0.1:8080/runs',data=json.dumps(item).encode(),headers={{'Content-Type':'application/json'}},method='POST')
 response=urllib.request.urlopen(request,timeout=60)
 assert response.status==202
print(json.dumps({{'submitted':len(requests)}}))
''',120)
    # Exercise recovery while work is in flight. Durable plans and the result inbox survive.
    remote("import subprocess,json\nsubprocess.run(['docker','restart','controller'],check=True,stdout=subprocess.DEVNULL)\nprint(json.dumps({'restarted':True}))",60)
    print("Controller restarted while runs were in flight; awaiting completion", flush=True)
    ids={name:d["run_id"] for name,d in documents.items()}
    encoded_ids=base64.b64encode(json.dumps(ids).encode()).decode()
    code=f'''import json,base64,time,urllib.request
ids=json.loads(base64.b64decode({encoded_ids!r}))
responses={{}}
for name,run in ids.items():
 with urllib.request.urlopen('http://127.0.0.1:8080/runs/'+run,timeout=10) as response: value=json.load(response)
 for node in value['nodes']:
  if node['result']: node['result']={{'result':node['result']['result'],'processes':node['result']['processes'],'error':node['result'].get('error')}}
 responses[name]=value
print(json.dumps(responses))
'''
    end=time.monotonic()+600
    observed={}
    while time.monotonic()<end:
        try: observed=remote(code,90)
        except RuntimeError:
            time.sleep(3);continue
        if any(v["state"]=="cancelled" for v in observed.values()):
            failures={k:[{"node":n["name"],"error":n["result"].get("error") if n["result"] else None} for n in v["nodes"]] for k,v in observed.items() if v["state"]=="cancelled"}
            raise RuntimeError("cancelled scenario: "+json.dumps(failures))
        if all(v["state"]=="completed" for v in observed.values()):break
        time.sleep(3)
    else:raise TimeoutError("judge runs did not complete")
    def result(scenario,node):
        return next(n["result"]["result"] for n in observed[scenario]["nodes"] if n["name"]==node)
    assert result("batch","check")["verdict"]=="accepted"
    assert result("heuristic","aggregate")=={"score":33.0,"trials":2}
    assert result("run_twice","check")["passes"]==2
    assert result("interactive","dialogue")["answer"]=="42"
    assert result("coding_game","match")=={"winner":"b","scores":{"a":4,"b":5}}
    assert result("timeout","timeout")["verdict"]=="time_limit_exceeded"
    assert observed["mixed_cases"]["completed_case_count"]==3
    assert {n["result"]["result"]["verdict"] for n in observed["mixed_cases"]["nodes"]}=={"accepted","wrong_answer","time_limit_exceeded"}
    assert all(v["completed_case_count"]==v["total_case_count"] for v in observed.values())
    print("Seven scenarios completed with expected results; checking replay and corruption", flush=True)
    # Replay an accepted node result. The inbox and Engine must not release a second successor.
    replay=remote("import json,urllib.request\nwith urllib.request.urlopen('http://127.0.0.1:8080/runs/"+ids["batch"]+"') as r: value=json.load(r)\nprint(json.dumps(value['nodes'][0]['result']))")
    sqs.send_message(QueueUrl=outputs["ResultQueueUrl"],MessageBody=json.dumps(replay))
    remote(f'''import base64,json,urllib.request
item=json.loads(base64.b64decode({base64.b64encode(json.dumps(documents['batch']).encode()).decode()!r}))
r=urllib.request.Request('http://127.0.0.1:8080/runs',data=json.dumps(item).encode(),headers={{'Content-Type':'application/json'}})
assert urllib.request.urlopen(r).status==202
print(json.dumps({{'replayed':True}}))
''')
    time.sleep(3)
    replayed=remote(code,90)
    for scenario,v in observed.items():
        assert len(v["nodes"])==len(replayed[scenario]["nodes"])
        assert replayed[scenario]["completed_case_count"]==v["completed_case_count"]
    # A wrong checksum is rejected before queue dispatch.
    bad=json.loads(json.dumps(documents["batch"]))
    bad["run_id"]=str(uuid.uuid4())
    next(a for a in bad["definition"]["artifacts"] if a["reference"])["reference"]["sha256"]="0"*64
    remote(f'''import json,base64,urllib.request,urllib.error
item=json.loads(base64.b64decode({base64.b64encode(json.dumps(bad).encode()).decode()!r}))
try:
 urllib.request.urlopen(urllib.request.Request('http://127.0.0.1:8080/runs',data=json.dumps(item).encode(),headers={{'Content-Type':'application/json'}}))
 raise AssertionError('corrupted input accepted')
except urllib.error.HTTPError as error: assert error.code==400
print(json.dumps({{'checksum_rejected':True}}))
''')
    report={"schema_version":1,"aws_verified":True,"checks":{"internal_api_healthy":True,"pinned_input_survives_key_replacement":True,"controller_restart":True,"duplicate_result":True,"duplicate_submission":True,"corrupt_checksum_rejected":True},"scenarios":{name:{"state":v["state"],"completed_cases":v["completed_case_count"],"total_cases":v["total_case_count"],"nodes":[{"name":n["name"],"state":n["state"],"result":n["result"]["result"],"processes":n["result"]["processes"]} for n in v["nodes"]],"events":v["events"]} for name,v in observed.items()}}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2))
    print(json.dumps({"verified":list(report["scenarios"]),"checks":report["checks"]}))

if __name__=="__main__":main()
