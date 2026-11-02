"""IAM-invoked probe ingress / recovery tick, outside the zero-sized EC2 pool."""
import json
import os
from pathlib import Path
import subprocess
import urllib.parse
import uuid
import ssl
import contextlib
import pg8000.native

import boto3
from botocore.config import Config

initialized=False

@contextlib.contextmanager
def scaling_lock():
    tls=ssl.create_default_context(cafile=str(Path(__file__).parent/'rds-ca.pem'))
    connection=pg8000.native.Connection(user=os.environ['DB_USER'],password=os.environ['DB_PASSWORD'],
        host=os.environ['DB_HOST'],database='judge',ssl_context=tls,timeout=10)
    # Bound the lifetime of a lock left behind by a frozen/crashed control process.
    try:
        connection.run("SET idle_session_timeout='90s'")
        connection.run("SET lock_timeout='5s'")
        connection.run('SELECT pg_advisory_lock(44003)')
        yield connection
    finally:connection.close()


def run(*args):
    root=Path(__file__).parent
    environment=os.environ.copy()
    environment['DATABASE_URL']='postgresql://'+urllib.parse.quote(os.environ['DB_USER'],safe='')+':'+urllib.parse.quote(os.environ['DB_PASSWORD'],safe='')+'@'+os.environ['DB_HOST']+':5432/judge?sslmode=verify-full&sslrootcert='+urllib.parse.quote(str(root/'rds-ca.pem'),safe='/')
    result=subprocess.run([str(root/'lib/ld-linux-x86-64.so.2'),'--library-path',str(root/'lib'),str(root/'judge-probe'),*args],env=environment,text=True,capture_output=True,timeout=25)
    if result.returncode:
        # Do not return connection strings or stderr containing credentials.
        message=result.stderr.replace(environment['DATABASE_URL'],'<database>').replace(os.environ['DB_PASSWORD'],'<password>')
        print('Rust command failed: '+args[0]+': '+message[:4096])
        raise RuntimeError('Rust probe command failed: '+args[0])
    return json.loads(result.stdout) if result.stdout.strip() else None


def handler(event,context):
    global initialized
    if not initialized:
        run('init')
        initialized=True
    action=event.get('action','tick')
    if action=='events':
        return run('events',event.get('scope','global'),'--after',str(int(event.get('after',0))))
    with scaling_lock() as connection:
        return handle_control(event,action,connection)


def handle_control(event,action,connection):
    receipt=None
    if action=='init':
        run('init')
    elif action=='submit':
        key=str(uuid.UUID(event['id']))
        fixture=Path('/tmp')/(key+'.json')
        encoded=json.dumps(event['fixture'])
        if len(encoded)>262144:raise ValueError('fixture cap')
        fixture.write_text(encoded)
        try:receipt=run('submit',key,event['scope'],str(fixture))
        finally:fixture.unlink(missing_ok=True)
    elif action not in ['tick','status']:
        raise ValueError('unknown action')
    run('tick')
    state=run('status')
    autoscaling=boto3.client('autoscaling',config=Config(connect_timeout=2,read_timeout=4,retries={'total_max_attempts':1}))
    group=autoscaling.describe_auto_scaling_groups(AutoScalingGroupNames=[os.environ['ASG']])['AutoScalingGroups'][0]
    active=sum(not item['terminal'] for item in state['submissions'])
    desired=min(5,max(group['DesiredCapacity'],active)) if active else 0
    if desired!=group['DesiredCapacity']:
        # Fail closed if the DB session (and therefore its lock) was lost.
        connection.run('SELECT 1')
        autoscaling.set_desired_capacity(AutoScalingGroupName=os.environ['ASG'],DesiredCapacity=desired,HonorCooldown=False)
    return {'receipt':receipt,'desired_capacity':desired,'state':state}
