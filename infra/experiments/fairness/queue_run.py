"""End-to-end SQS latency from compute zero, plus before/after-save failures and DLQ."""
import json
from pathlib import Path
import time
import uuid

import boto3
from botocore.exceptions import ClientError

from run import Queued, stats


def main():
    root = Path(__file__).resolve().parents[3]
    outputs = next(iter(json.loads((root / 'work/fairness-queue-outputs.json').read_text()).values()))
    managed = next(iter(json.loads((root / 'work/fairness-outputs.json').read_text()).values()))
    vm = boto3.client('lambda-microvms', region_name='ap-northeast-1')
    s3 = boto3.client('s3', region_name='ap-northeast-1')
    sqs = boto3.client('sqs', region_name='ap-northeast-1')
    identifiers = [managed[f'VM{i}Id'] for i in range(3)]
    for identifier in identifiers:
        if vm.get_microvm(microvmIdentifier=identifier)['state'] == 'RUNNING':
            vm.suspend_microvm(microvmIdentifier=identifier)

    def await_zero():
        start = time.monotonic()
        while time.monotonic() - start < 90:
            states = [vm.get_microvm(microvmIdentifier=i)['state'] for i in identifiers]
            if states == ['SUSPENDED'] * 3:
                return states
            time.sleep(.2)
        raise TimeoutError('Compute did not return to zero')

    report = {'definition': 'Zero RUNNING VMs; three suspended VM snapshots remain.',
        'initial_states': await_zero(), 'submissions': [], 'faults': {},
        'scope': 'One serialized slot; one allowlisted C fixture. Includes compilation, SQS, Lambda, resume and S3 polling.'}
    path = root / 'work/fairness-queue.json'

    def save():
        path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')

    client = Queued(outputs, 0)
    for i in range(30):
        await_zero()
        result = client.run({'compile': True, 'samples': 1, 'mode': 0, 'iterations': 100_000_000})
        if result['samples'][0]['verdict'] != 'OK':
            raise RuntimeError('A normal queued submission failed')
        report['submissions'].append(result); save()
        print('zero -> result', i + 1, round(result['client_seconds'], 3), flush=True)
    report['latency'] = stats([r['client_seconds'] for r in report['submissions']])
    report['cpu'] = stats([r['samples'][0]['cpu_seconds'] for r in report['submissions']])
    report['final_idle_states'] = await_zero(); save()

    for fault in ['before_result', 'after_result']:
        result = client.run({'compile': True, 'samples': 1, 'iterations': 100_000_000}, fault=fault)
        report['faults'][fault] = result; save()
        print('fault', fault, 'result observed', flush=True)
    identifier = report['faults']['after_result']['request_label']
    start = time.monotonic()
    while time.monotonic() - start < 120:
        try:
            obj = s3.get_object(Bucket=outputs['Bucket'], Key=f'repairs/{identifier}.json')
            report['faults']['after_result_ack_repaired'] = json.loads(obj['Body'].read())
            break
        except ClientError as exc:
            if exc.response['Error']['Code'] not in ['NoSuchKey', '404']:
                raise
            time.sleep(.5)
    else:
        raise TimeoutError('Missing replay ACK repair')
    save()

    poison = uuid.uuid4().hex
    message = {'id': poison, 'payload': {'samples': 1, 'iterations': 10_000_000}, 'fault': 'poison'}
    start = time.monotonic()
    sqs.send_message(QueueUrl=outputs['Queue'], MessageGroupId='slot-0',
        MessageDeduplicationId=poison, MessageBody=json.dumps(message))
    while time.monotonic() - start < 240:
        records = sqs.receive_message(QueueUrl=outputs['DLQ'], WaitTimeSeconds=5,
            MessageSystemAttributeNames=['ApproximateReceiveCount']).get('Messages', [])
        for record in records:
            if json.loads(record['Body'])['id'] != poison:
                raise RuntimeError('Unexpected DLQ message')
            report['faults']['poison_dlq'] = {'seconds': time.monotonic() - start,
                'original_message': json.loads(record['Body']), 'infrastructure_failure': True}
            sqs.delete_message(QueueUrl=outputs['DLQ'], ReceiptHandle=record['ReceiptHandle'])
            report['final_states'] = await_zero(); save()
            print('poison reached DLQ', flush=True)
            return
    raise TimeoutError('Poison did not reach DLQ')


if __name__ == '__main__':
    main()
