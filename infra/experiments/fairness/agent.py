"""One submission at a time, one gVisor container per submission, CPU 1 only."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import threading
import time
import uuid

import boto3
from botocore.exceptions import ClientError

s3 = boto3.client('s3', region_name='ap-northeast-1')
sqs = boto3.client('sqs', region_name='ap-northeast-1')
BUCKET, QUEUE = os.environ['BUCKET'], os.environ['QUEUE']
WORKER = uuid.uuid4().hex
os.sched_setaffinity(0, {0})


def store(key, value):
    return s3.put_object(Bucket=BUCKET, Key=key, Body=json.dumps(value).encode())


def consume(record):
    message = json.loads(record['Body'])
    identifier = message['id']
    if not re.fullmatch('[a-zA-Z0-9-]{1,80}', identifier):
        raise ValueError('Invalid identifier')
    key = f'results/{identifier}.json'
    try:
        s3.head_object(Bucket=BUCKET, Key=key)
    except ClientError as exc:
        if exc.response['Error']['Code'] not in ['404', 'NoSuchKey']:
            raise
    else:
        sqs.delete_message(QueueUrl=QUEUE, ReceiptHandle=record['ReceiptHandle'])
        return
    if message.get('fault') == 'poison':
        sqs.change_message_visibility(QueueUrl=QUEUE, ReceiptHandle=record['ReceiptHandle'], VisibilityTimeout=1)
        return
    if message.get('fault') in ['before_result', 'after_result']:
        try:
            s3.put_object(Bucket=BUCKET, Key=f'faults/{identifier}.json', Body=b'{}', IfNoneMatch='*')
            crash = True
        except ClientError as exc:
            if exc.response['Error']['Code'] != 'PreconditionFailed':
                raise
            crash = False
        if crash and message['fault'] == 'before_result':
            os._exit(77)
    else:
        crash = False
    stop = threading.Event()

    def renew():
        while not stop.wait(15):
            sqs.change_message_visibility(QueueUrl=QUEUE, ReceiptHandle=record['ReceiptHandle'], VisibilityTimeout=60)

    threading.Thread(target=renew, daemon=True).start()
    name = 'fairness-' + uuid.uuid4().hex
    runtime = message.get('runtime', 'runsc')
    if runtime not in ['runsc', 'runc']:
        raise ValueError('Only allowlisted diagnostic runtimes are supported')
    command = ['docker', 'run', '--name', name, '--label=fairness.managed=true',
        '--runtime=' + runtime, '--network=none', '--read-only', '--cpuset-cpus=1',
        '--pids-limit=64', '--memory=512m', '--memory-swap=512m',
        '--security-opt=no-new-privileges', '--cap-drop=ALL', '--cap-add=SETUID', '--cap-add=SETGID', '--cap-add=KILL',
        '--tmpfs=/tmp:rw,nosuid,noexec,size=64m,nr_inodes=2048,mode=1777',
        # Compiled fixture binaries execute from this bounded tmpfs, never a host bind.
        '--tmpfs=/build:rw,exec,nosuid,size=32m,nr_inodes=2048,mode=1777',
        '-e', 'TMPDIR=/build', '-i', 'fairness:proof',
        'python3', '/app/server.py', '--job']
    try:
        started = time.monotonic()
        result = subprocess.run(command, input=json.dumps(message['payload']), capture_output=True, text=True, timeout=120)
        config = json.loads(subprocess.check_output(['docker', 'inspect', '--format={{json .HostConfig}}', name]))
        if config['Runtime'] != runtime or config['NetworkMode'] != 'none' or config['CpusetCpus'] != '1':
            raise RuntimeError('Isolation configuration mismatch')
        value = json.loads(result.stdout) if result.returncode == 0 else {'infrastructure_error': 'container_failed'}
        # Infrastructure failures are explicit IE, never contestant TLE.
        value.update(worker=WORKER, container_seconds=time.monotonic() - started,
            receive_count=int(record.get('Attributes', {}).get('ApproximateReceiveCount', '1')),
            isolation={'runtime': config['Runtime'], 'network': config['NetworkMode'],
                       'cpuset': config['CpusetCpus'], 'memory_bytes': config['Memory']})
        if result.returncode:
            value['error_excerpt'] = result.stderr[-600:]
        s3.put_object(Bucket=BUCKET, Key=key, Body=json.dumps(value).encode(), IfNoneMatch='*')
        if crash and message['fault'] == 'after_result':
            os._exit(78)
        sqs.delete_message(QueueUrl=QUEUE, ReceiptHandle=record['ReceiptHandle'])
    finally:
        stop.set()
        subprocess.run(['docker', 'rm', '-f', name], capture_output=True)


def main():
    orphans = subprocess.check_output(['docker', 'ps', '-aq', '--filter=label=fairness.managed=true'], text=True).split()
    if orphans:
        subprocess.run(['docker', 'rm', '-f', *orphans], check=True, capture_output=True)
    model = sorted(set(s.strip() for s in Path('/proc/cpuinfo').read_text().splitlines()
        if s.startswith(('model name', 'CPU implementer', 'CPU part', 'CPU variant', 'CPU revision'))))
    store(f'hosts/{WORKER}.json', {'worker': WORKER, 'ready_at': time.time(), 'cpu_model': model,
        'architecture': os.uname().machine, 'cpus': os.cpu_count(),
        'runtime_version': subprocess.check_output(['runsc', '--version'], text=True).splitlines()[0],
        'image_id': subprocess.check_output(['docker', 'image', 'inspect', '--format={{.Id}}', 'fairness:proof'], text=True).strip()})
    while True:
        for record in sqs.receive_message(QueueUrl=QUEUE, WaitTimeSeconds=5, MaxNumberOfMessages=1,
                MessageSystemAttributeNames=['ApproximateReceiveCount']).get('Messages', []):
            try:
                consume(record)
            except Exception as exc:
                print(json.dumps({'infrastructure_error': type(exc).__name__}), flush=True)


if __name__ == '__main__':
    main()
