"""Run a phased, disposable AWS workload proof. Keep raw resource IDs in work/."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import statistics
import time
import uuid
import threading

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

ROOT=Path(__file__).resolve().parents[3]
WORK=ROOT/'work'
SESSION=boto3.Session(profile_name='AWS',region_name='ap-northeast-1')
CONFIG=Config(connect_timeout=10,read_timeout=70,retries={'max_attempts':3,'mode':'standard'})


class Probe:
    def __init__(self):
        self.lock=threading.RLock()
        self.outputs=next(iter(json.loads((WORK/'pg-pool-outputs.json').read_text()).values()))
        self.group=self.outputs['ASG'];self.bucket=self.outputs['Bucket'];self.function=self.outputs['ControllerName']
        self.aws={name:SESSION.client(name,config=CONFIG) for name in ['lambda','autoscaling','ssm','s3']}
        self.file=WORK/'pg-pool-run-state.json'
        self.state=json.loads(self.file.read_text()) if self.file.exists() else {'receipts':{},'events':[]}

    def save(self):
        with self.lock:
            temporary=self.file.with_suffix('.tmp')
            temporary.write_text(json.dumps(self.state,indent=2));temporary.replace(self.file)
    def event(self,name,**data):
        record={'event':name,'time':time.time(),**data};self.state['events'].append(record);self.save()
        print(json.dumps(record),flush=True)

    def invoke(self,event):
        response=self.aws['lambda'].invoke(FunctionName=self.function,Payload=json.dumps(event).encode())
        data=json.loads(response['Payload'].read())
        if response.get('FunctionError'):
            (WORK/'pg-pool-lambda-error.json').write_text(json.dumps(data,indent=2))
            raise RuntimeError('Lambda failed; details saved privately')
        return data

    def submit(self,label,fixture):
        with self.lock:
            if label not in self.state['receipts']:
                self.state['receipts'][label]={'id':str(uuid.uuid4()),'fixture':fixture}
                self.save()
            item=self.state['receipts'][label]
        assert item['fixture']==fixture,'retry fixture changed'
        with self.lock:
            item.setdefault('client_submit_at',time.time());self.save()
        start=time.monotonic()
        result=self.invoke({'action':'submit','id':item['id'],'scope':'global','fixture':fixture})
        with self.lock:
            item['ordinal']=result['receipt']['ordinal'];item['request_seconds']=time.monotonic()-start
            self.save()
        return item['id']

    def poll_events(self):
        cursor=self.state.get('cursor',0)
        while True:
            page=self.invoke({'action':'events','after':cursor})
            assert all(e['cursor']>cursor for e in page['events'])
            with self.lock:
                for event in page['events']:
                    if event['kind']=='result.published':
                        for item in self.state['receipts'].values():
                            if item['id']==event['submission']:
                                item.setdefault('client_result_seconds',time.time()-item['client_submit_at'])
                                item['event_verdict']=event['payload']['verdict']
                self.state.setdefault('feed',[]).extend(page['events'])
                self.state['cursor']=page['cursor'];self.save();cursor=page['cursor']
            if len(page['events'])<256:return

    def wait(self,labels,timeout=300,interval=2,public=False):
        ordinals={self.state['receipts'][label]['ordinal'] for label in labels}
        start=time.monotonic()
        while time.monotonic()-start < timeout:
            data=self.invoke({'action':'status'})
            self.poll_events()
            rows=[row for row in data['state']['submissions'] if row['scope']=='global' and row['ordinal'] in ordinals]
            if len(rows)==len(labels) and all(row['terminal'] for row in rows):
                if not public or all('event_verdict' in self.state['receipts'][label] for label in labels):return data
            time.sleep(interval)
        raise TimeoutError('receipts did not reach a terminal state')

    def hosts(self):
        group=self.aws['autoscaling'].describe_auto_scaling_groups(AutoScalingGroupNames=[self.group])['AutoScalingGroups'][0]
        assert group['MaxSize']==5 and len(group['Instances'])<=5
        return [i['InstanceId'] for i in group['Instances'] if i['LifecycleState']=='InService']

    def command(self,host,commands,timeout=90):
        prefix='commands/'+str(uuid.uuid4())
        response=self.aws['ssm'].send_command(InstanceIds=[host],DocumentName='AWS-RunShellScript',
            Parameters={'commands':['set -e']+commands,'executionTimeout':[str(timeout)]},
            OutputS3BucketName=self.bucket,OutputS3KeyPrefix=prefix)
        command=response['Command']['CommandId'];start=time.monotonic()
        while time.monotonic()-start<timeout+15:
            try:
                result=self.aws['ssm'].get_command_invocation(CommandId=command,InstanceId=host)
            except ClientError as error:
                if error.response['Error']['Code']!='InvocationDoesNotExist':raise
                time.sleep(.5);continue
            if result['Status'] in ['Success','Failed','Cancelled','TimedOut']:
                if result['Status']!='Success':
                    (WORK/'pg-pool-ssm-error.json').write_text(json.dumps(result,indent=2))
                    raise RuntimeError('SSM command failed; details saved privately')
                for _ in range(20):
                    objects=self.aws['s3'].list_objects_v2(Bucket=self.bucket,Prefix=prefix).get('Contents',[])
                    stdout=[obj['Key'] for obj in objects if obj['Key'].endswith('/stdout')]
                    if stdout:return self.aws['s3'].get_object(Bucket=self.bucket,Key=stdout[0])['Body'].read().decode()
                    time.sleep(.2)
                return result['StandardOutputContent']
            time.sleep(.5)
        raise TimeoutError('SSM command did not complete')

    def collect(self):
        host=self.hosts()[0]
        self.aws['s3'].upload_file(str(WORK/'pg-pool-inspector.zip'),self.bucket,'tools/inspector.zip')
        result=self.command(host,[f'aws s3 cp s3://{self.bucket}/tools/inspector.zip /opt/judge-pool/inspector.zip --region ap-northeast-1 >/dev/null',
            'mkdir -p /opt/judge-pool/inspector',
            'python3 -m zipfile -e /opt/judge-pool/inspector.zip /opt/judge-pool/inspector',
            'docker cp /opt/judge-pool/inspector/. judge-control:/opt/inspector',
            'docker exec judge-control /opt/tools/bin/python3 /opt/inspector/collect.py'])
        data=json.loads(result);(WORK/'pg-pool-collected.json').write_text(json.dumps(data,indent=2));return data

    def assert_verdicts(self,labels,expected='AC'):
        data=self.collect();ids={self.state['receipts'][label]['id'] for label in labels}
        checks=[node for node in data['nodes'] if node['pipeline_id'] in ids and node['phase']=='check']
        assert len(checks)==len(labels),(len(checks),len(labels))
        assert all(node['verdict']==expected for node in checks),checks
        return data

    def cold(self):
        self.invoke({'action':'init'})
        assert not self.hosts(),'cold test needs zero instances'
        # Freeze capacity for warm measurements, then explicitly resume and
        # verify zero again. This is not a claim that a warm pool is free.
        self.aws['autoscaling'].suspend_processes(AutoScalingGroupName=self.group,ScalingProcesses=['Terminate'])
        self.state['terminate_suspended']=True;self.save()
        labels=[];start=time.monotonic()
        for i,language in enumerate(['cpp','python','rust']*2):
            label=f'cold-{i}';labels.append(label);self.submit(label,hello(language))
        self.wait(labels,timeout=330,interval=5,public=True)
        data=self.assert_verdicts(labels)
        assert len(self.hosts())==5
        self.event('cold_complete',observed_seconds=time.monotonic()-start,instances=5,
            ordered_seconds=[r['ordered_seconds'] for r in data['submissions']],
            client_result_seconds=[self.state['receipts'][label]['client_result_seconds'] for label in labels])

    def warm(self):
        labels=[]
        for i,language in enumerate(['cpp','python','rust']*2):
            label=f'warm-fixed-{i}';labels.append(label);self.submit(label,hello(language))
            self.wait([label],public=True,interval=.1)
        self.wait(labels,public=True);data=self.assert_verdicts(labels)
        ids={self.state['receipts'][label]['id'] for label in labels}
        self.event('warm_complete',ordered_seconds=[r['ordered_seconds'] for r in data['submissions'] if r['id'] in ids],
            publication_seconds=[r['publication_seconds'] for r in data['submissions'] if r['id'] in ids],
            client_result_seconds=[self.state['receipts'][label]['client_result_seconds'] for label in labels])

    def burst(self):
        labels=[f'burst-{i}' for i in range(5)];stop=threading.Event();errors=[]
        def poll():
            try:
                while not stop.is_set():self.poll_events();time.sleep(.1)
            except Exception as error:errors.append(error)
        reader=threading.Thread(target=poll);reader.start()
        try:
            with ThreadPoolExecutor(max_workers=5) as executor:
                list(executor.map(lambda label:self.submit(label,hello('cpp')),labels))
        finally:
            stop.set();reader.join(30)
        if errors:raise errors[0]
        assert not reader.is_alive()
        self.wait(labels,public=True,interval=.1);self.assert_verdicts(labels)
        self.event('burst_complete',client_result_seconds=[self.state['receipts'][label]['client_result_seconds'] for label in labels])

    def events(self):
        self.poll_events();saved=self.state['feed'];cursor=0;replayed=[]
        first=self.invoke({'action':'events','after':0})
        again=self.invoke({'action':'events','after':0})
        assert again['events'][:len(first['events'])]==first['events']
        while True:
            page=self.invoke({'action':'events','after':cursor});replayed+=page['events'];cursor=page['cursor']
            if len(page['events'])<256:break
        assert replayed[:len(saved)]==saved
        assert [e['cursor'] for e in replayed]==list(range(1,len(replayed)+1))
        published=[e for e in replayed if e['kind']=='result.published']
        ordinals=[e['payload']['ordinal'] for e in published];assert ordinals==sorted(set(ordinals))
        self.state['replayed_feed']=replayed;self.save()
        self.event('events_reconnect',events=len(replayed),published=len(published))

    def fault(self):
        fixture={'language':'python','source':'import time\ntime.sleep(4)\nprint(42)','input':'','expected':'42\n'}
        hosts=self.hosts()
        watcher="""import subprocess,time,pathlib,json
+end=time.monotonic()+15
+while time.monotonic()<end:
+ ids=subprocess.check_output(['docker','ps','-q','--filter','label=traojudge.probe=true'],text=True).split()
+ for cid in ids:
+  try:info=json.loads(subprocess.check_output(['docker','inspect',cid],text=True))[0]
+  except subprocess.CalledProcessError:continue
+  if info.get('ExecIDs'):
+   subprocess.run(['docker','pause','judge-control'],check=True,timeout=5)
+   pathlib.Path('/opt/judge-pool/fault-paused').write_text('yes')
+   raise SystemExit(0)
+ time.sleep(.05)
+""".replace('\n+','\n')
        def arm(host):
            self.command(host,["cat > /opt/judge-pool/fault-watch.py <<'PY'",watcher,'PY',
                'rm -f /opt/judge-pool/fault-paused',
                'nohup python3 /opt/judge-pool/fault-watch.py >/opt/judge-pool/fault-watch.log 2>&1 </dev/null &'])
        with ThreadPoolExecutor(max_workers=5) as executor:list(executor.map(arm,hosts))
        self.submit('worker-loss-active',fixture)
        try:
            time.sleep(12)
            paused=[]
            for host in hosts:
                status=json.loads(self.command(host,["python3 - <<'PY'",'import json,subprocess,pathlib',
                    "print(json.dumps({'paused':pathlib.Path('/opt/judge-pool/fault-paused').exists(),'containers':len(subprocess.check_output(['docker','ps','-aq','--filter','label=traojudge.probe=true'],text=True).split()),'job_roots':len(list(pathlib.Path('/var/lib/judge-probe/jobs').iterdir()))}))",'PY']))
                if status['paused']:paused.append(status)
            assert len(paused)==1 and paused[0]['containers']==0 and paused[0]['job_roots']==0,paused
            self.event('orphan_reaped_while_worker_paused',containers=0,job_roots=0)
        finally:
            for host in hosts:self.command(host,['docker unpause judge-control >/dev/null 2>&1 || true','systemctl restart judge-worker'])
        self.wait(['worker-loss-active'],timeout=120)
        data=self.assert_verdicts(['worker-loss-active'])
        key=self.state['receipts']['worker-loss-active']['id']
        attempts=[n['attempt'] for n in data['nodes'] if n['pipeline_id']==key and n['phase']=='run']
        assert attempts==[2],attempts
        self.event('worker_loss_recovered',attempts=attempts)
        self.submit('poison',{'language':'python','source':'import time\ntime.sleep(60)','input':'','expected':''})
        self.submit('after-poison',hello('python'))
        self.wait(['after-poison'],timeout=40)
        interim=self.collect();following=self.state['receipts']['after-poison']['id']
        row=next(r for r in interim['submissions'] if r['id']==following)
        assert row['terminal'] and not row['published']
        self.wait(['poison','after-poison'],timeout=150)
        data=self.collect();poison=self.state['receipts']['poison']['id']
        rows=[n for n in data['nodes'] if n['pipeline_id']==poison and n['phase']=='run']
        assert len(rows)==1 and rows[0]['status']=='dead' and rows[0]['attempt']==3,rows
        assert next(r for r in data['submissions'] if r['id']==following)['published']
        self.event('poison_dlq_and_ordered_release',attempts=3)

    def security(self):
        network="""import socket,os
assert not any(k in os.environ for k in ['DATABASE_URL','AWS_ACCESS_KEY_ID','AWS_SESSION_TOKEN'])
assert not os.path.exists('/var/run/docker.sock')
for address,port in [('169.254.169.254',80),('1.1.1.1',443)]:
 try:
  connection=socket.create_connection((address,port),timeout=.2)
 except OSError:continue
 else:raise RuntimeError('network reached')
print('BLOCKED')
"""
        process="""import os,signal,time
children=[];limited=False
try:
 for i in range(150):
  try:pid=os.fork()
  except OSError:limited=True;break
  if pid==0:
   time.sleep(60);os._exit(0)
  children.append(pid)
finally:
 for pid in children:os.kill(pid,signal.SIGKILL)
 for pid in children:os.waitpid(pid,0)
print('LIMITED' if limited else 'UNBOUNDED')
"""
        cases=[('network',network,'BLOCKED\n'),('process',process,'LIMITED\n')]
        for label,source,expected in cases:
            self.submit(label,{'language':'python','source':source,'input':'','expected':expected})
            self.wait([label]);self.assert_verdicts([label]);self.event(label+'_guard')
        self.submit('invalid-utf8',{'language':'python','source':"import sys;sys.stdout.buffer.write(b'\\xff')",'input':'','expected':'�'})
        self.wait(['invalid-utf8']);self.assert_verdicts(['invalid-utf8'],'WA')
        self.submit('output-limit',{'language':'python','source':"print('x'*1000000)",'input':'','expected':''})
        self.wait(['output-limit']);self.assert_verdicts(['output-limit'],'OLE')
        self.event('byte_checker_and_output_limit')
        self.submit('memory-limit',{'language':'python','source':'a=[]\nwhile True:a.append(bytearray(16*1024*1024))','input':'','expected':''})
        self.submit('after-memory',hello('python'))
        self.wait(['memory-limit','after-memory'],timeout=160)
        self.assert_verdicts(['after-memory'])
        data=self.collect();key=self.state['receipts']['memory-limit']['id']
        rows=[n for n in data['nodes'] if n['pipeline_id']==key and n['phase']=='run']
        assert len(rows)==1 and rows[0]['status']=='done' and rows[0]['verdict']=='MLE',rows
        self.event('memory_bomb_terminal',status=rows[0]['status'],verdict=rows[0]['verdict'])

    def fairness(self,count=20):
        labels=[]
        for i in range(count):
            label=f'serial-{i}';labels.append(label);self.submit(label,arithmetic(80_000_000));self.wait([label]);self.assert_verdicts([label])
        for batch in range(0,count,5):
            current=[f'parallel-{i}' for i in range(batch,min(batch+5,count))]
            with ThreadPoolExecutor(max_workers=5) as executor:list(executor.map(lambda label:self.submit(label,arithmetic(80_000_000)),current))
            self.wait(current);self.assert_verdicts(current);labels+=current
        data=self.collect()
        def runtimes(prefix):
            ids={v['id'] for k,v in self.state['receipts'].items() if k.startswith(prefix)}
            return [n['host_cpu_usec'] for n in data['nodes'] if n['pipeline_id'] in ids and n['phase']=='run']
        serial=runtimes('serial-');parallel=runtimes('parallel-')
        assert len(serial)==count and len(parallel)==count
        self.event('fairness_distribution',serial=distribution(serial),parallel=distribution(parallel))
        boundary=max(1,round(80_000_000*3_000_000/statistics.median(serial)))
        current=[]
        for batch in range(0,20,5):
            group=[f'boundary-{i}' for i in range(batch,batch+5)]
            for label in group:self.submit(label,arithmetic(boundary))
            self.wait(group);current+=group
        data=self.collect();ids={self.state['receipts'][label]['id'] for label in current}
        verdicts=[n['verdict'] for n in data['nodes'] if n['pipeline_id'] in ids and n['phase']=='check']
        assert len(verdicts)==20,verdicts
        assert all(v in ['AC','CPU_LIMIT'] for v in verdicts),verdicts
        self.event('cpu_limit_boundary',iterations=boundary,verdicts=verdicts)

    def finish(self):
        data=self.collect();self.state['final']=data;self.save()
        hosts=self.hosts();metadata=[]
        for host in hosts:
            metadata.append(json.loads(self.command(host,["python3 - <<'PY'",'import subprocess,json,pathlib',
                "cpu=json.loads(subprocess.check_output(['lscpu','-J'],text=True))",
                "print(json.dumps({'cpu':cpu,'gvisor':subprocess.check_output(['runsc','--version'],text=True),'containers':subprocess.check_output(['docker','ps','-aq','--filter','label=traojudge.probe=true'],text=True).split(),'roots':len(list(pathlib.Path('/var/lib/judge-probe/jobs').iterdir()))}))",'PY'])))
        self.state['hosts_metadata']=metadata;self.save()
        assert all(not m['containers'] and m['roots']==0 for m in metadata)
        self.aws['autoscaling'].resume_processes(AutoScalingGroupName=self.group,ScalingProcesses=['Terminate'])
        self.state['terminate_suspended']=False;self.save()
        self.invoke({'action':'tick'})
        start=time.monotonic()
        while self.hosts():
            if time.monotonic()-start>240:raise TimeoutError('pool did not reach zero')
            time.sleep(5)
        self.event('scale_to_zero',seconds=time.monotonic()-start)


def hello(language):
    return {'language':language,'source':{'cpp':'#include <cstdio>\nint main(){puts("42");}',
        'python':'print(42)','rust':'fn main(){println!("42");}'}[language],'input':'','expected':'42\n'}


def arithmetic(iterations):
    return {'language':'cpp','source':f'''#include <cstdint>
#include <cstdio>
int main(){{volatile std::uint64_t x=0x12345678;for(std::uint64_t i=0;i<{iterations}ULL;++i){{auto v=x;x=(v^(v>>13))*6364136223846793005ULL+i;}}puts("42");}}
''','input':'','expected':'42\n'}


def distribution(values):
    return {'n':len(values),'median_usec':statistics.median(values),'min_usec':min(values),'max_usec':max(values),
        'cv_percent':statistics.pstdev(values)/statistics.mean(values)*100}


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('phase',choices=['cold','warm','burst','fault','security','fairness','events','finish'])
    parser.add_argument('--count',type=int,default=20);args=parser.parse_args();probe=Probe()
    if args.phase=='fairness':probe.fairness(args.count)
    else:getattr(probe,args.phase)()
