"""One durable submission message -> resume existing CFN-owned VM -> result -> suspend."""
import json
import os
import time

import boto3
from botocore.exceptions import ClientError
from provider import request, token

s3 = boto3.client('s3')
vm = boto3.client('lambda-microvms')
BUCKET, IDENTIFIER = os.environ['BUCKET'], os.environ['VM']


def suspend():
    state = vm.get_microvm(microvmIdentifier=IDENTIFIER)['state']
    if state == 'RUNNING':
        vm.suspend_microvm(microvmIdentifier=IDENTIFIER)


def handler(event, context):
    for record in event['Records']:
        message = json.loads(record['body'])
        identifier = message['id']
        if not identifier.isalnum() or len(identifier) > 80:
            raise ValueError('Invalid job identifier')
        key = f'results/{identifier}.json'
        try:
            s3.head_object(Bucket=BUCKET, Key=key)
        except ClientError as exc:
            if exc.response['Error']['Code'] not in ['NoSuchKey', '404']:
                raise
        else:
            s3.put_object(Bucket=BUCKET, Key=f'repairs/{identifier}.json',
                Body=json.dumps({'receive_count': int(record['attributes']['ApproximateReceiveCount'])}).encode())
            suspend()
            continue
        started = time.monotonic()
        state = vm.get_microvm(microvmIdentifier=IDENTIFIER)
        try:
            if state['state'] == 'SUSPENDED':
                vm.resume_microvm(microvmIdentifier=IDENTIFIER)
            deadline = time.monotonic() + 15
            while state['state'] != 'RUNNING':
                if time.monotonic() > deadline:
                    raise TimeoutError('Resume deadline exceeded')
                time.sleep(.1)
                state = vm.get_microvm(microvmIdentifier=IDENTIFIER)
            result = request(state['endpoint'], token(vm, IDENTIFIER), message['payload'])
            result.update(worker_seconds=time.monotonic() - started,
                          receive_count=int(record['attributes']['ApproximateReceiveCount']))
            if message.get('fault'):
                try:
                    s3.put_object(Bucket=BUCKET, Key=f'faults/{identifier}.json', Body=b'{}', IfNoneMatch='*')
                    first = True
                except ClientError as exc:
                    if exc.response['Error']['Code'] != 'PreconditionFailed':
                        raise
                    first = False
                if message['fault'] == 'poison' or (first and message['fault'] == 'before_result'):
                    raise RuntimeError('Injected infrastructure interruption before result')
            s3.put_object(Bucket=BUCKET, Key=key, Body=json.dumps(result).encode(), IfNoneMatch='*')
            if message.get('fault') == 'after_result' and first:
                raise RuntimeError('Injected interruption after durable result, before ACK')
        finally:
            suspend()
