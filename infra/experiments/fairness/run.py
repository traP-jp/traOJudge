"""Collect real AWS measurements; persist after each phase so failed runs stay reviewable."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import datetime
import importlib.util
import json
import math
from pathlib import Path
import statistics
import time
import uuid

import boto3
from botocore.exceptions import ClientError

from provider import request, token


def stats(values):
    values = sorted(values)
    return {'n': len(values), 'min': min(values), 'median': statistics.median(values),
            'p95': values[math.ceil(len(values) * .95) - 1], 'max': max(values),
            'cv': statistics.pstdev(values) / statistics.mean(values)}


class Managed:
    def __init__(self, outputs, index):
        self.label = f'microvm-{index}'
        self.identifier = outputs[f'VM{index}Id']
        self.endpoint = outputs[f'VM{index}Endpoint']
        self.client = boto3.client('lambda-microvms', region_name='ap-northeast-1')
        self.auth = token(self.client, self.identifier)

    def run(self, payload, **unused):
        start = time.monotonic()
        result = request(self.endpoint, self.auth, payload)
        result['client_seconds'] = time.monotonic() - start
        return result


class Queued:
    def __init__(self, outputs, index):
        self.label = f'queue-request-{index}'
        self.outputs = outputs
        self.s3 = boto3.client('s3', region_name='ap-northeast-1')
        self.sqs = boto3.client('sqs', region_name='ap-northeast-1')
        self.runtime = 'runsc'

    def run(self, payload, fault=None, on_enqueued=None):
        identifier = uuid.uuid4().hex
        start = time.monotonic()
        message = {'id': identifier, 'payload': payload, 'runtime': self.runtime}
        if fault:
            message['fault'] = fault
        params = {'MessageGroupId': 'slot-0', 'MessageDeduplicationId': identifier} if self.outputs.get('Fifo') == 'true' else {}
        self.sqs.send_message(QueueUrl=self.outputs['Queue'], MessageBody=json.dumps(message), **params)
        if on_enqueued:
            on_enqueued()
        while time.monotonic() - start < 360:
            try:
                obj = self.s3.get_object(Bucket=self.outputs['Bucket'], Key=f'results/{identifier}.json')
                result = json.loads(obj['Body'].read())
                result.update(client_seconds=time.monotonic() - start, request_label=identifier)
                if result.get('infrastructure_error'):
                    raise RuntimeError(json.dumps(result))
                return result
            except ClientError as exc:
                if exc.response['Error']['Code'] not in ['NoSuchKey', '404']:
                    raise
            time.sleep(.2)
        raise TimeoutError('No durable result before experiment deadline')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=['microvm', 'ec2'], required=True)
    parser.add_argument('--samples', type=int, default=20)
    parser.add_argument('--report', required=True)
    parser.add_argument('--precompiled', action='store_true', help='Execution-only cohort; excludes runtime compilation')
    parser.add_argument('--runtime', choices=['runsc', 'runc'], default='runsc')
    parser.add_argument('--quick-control', action='store_true', help='Measure workload distributions only')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    filename = 'fairness-outputs.json' if args.target == 'microvm' else 'fairness-ec2-outputs.json'
    outputs = next(iter(json.loads((root / 'work' / filename).read_text()).values()))
    clients = [Managed(outputs, i) for i in range(3)] if args.target == 'microvm' else [Queued(outputs, i) for i in range(2)]
    if args.target == 'ec2':
        for client in clients:
            client.runtime = args.runtime
    report = {'target': args.target, 'at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'criteria': {'p95_cpu_change_max': .05, 'warm_client_p95_seconds_max': 10,
                     'boundary_guard_band': .05, 'boundary_repetitions': args.samples},
        'compile_included': not args.precompiled,
        'runtime': args.runtime if args.target == 'ec2' else 'microvm-native',
        'measurements': [], 'notes': ['Allowlisted fixtures; no product API or general isolation certification.',
            'CPU includes the restricted launcher and measured reaped descendants.',
            'Client latency includes HTTPS or SQS and S3 polling, depending on target.']}

    def save():
        Path(args.report).write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')

    def record(phase, workload, client, payload):
        result = client.run(payload)
        if any(s['verdict'] != 'OK' for s in result['samples']) and not phase.startswith(('boundary', 'watchdog', 'abort')):
            raise RuntimeError('Fixture failed during a normal measurement')
        return {'phase': phase, 'workload': workload, 'client': client.label, 'result': result}

    workloads = [('cpu', 0, 100_000_000), ('memory', 2, 4_000_000), ('children', 1, 50_000_000)]
    for workload, mode, count in workloads:
        payload = {'mode': mode, 'iterations': count, 'samples': args.samples, 'compile': not args.precompiled}
        for client in clients:
            report['measurements'].append(record('sequential', workload, client, payload))
            save()
        with ThreadPoolExecutor(max_workers=len(clients)) as pool:
            results = list(pool.map(lambda c: record('parallel', workload, c, payload), clients))
        report['measurements'].extend(results)
        save()
        cpu = [s['cpu_seconds'] for m in results for s in m['result']['samples']]
        print(args.target, workload, 'parallel CPU', json.dumps(stats(cpu)), flush=True)

    if args.quick_control:
        print('Saved control', args.report, flush=True)
        return

    baseline = [s['cpu_seconds'] for m in report['measurements'] if m['phase'] == 'sequential' and m['workload'] == 'cpu'
                for s in m['result']['samples']]
    median = statistics.median(baseline)
    report['boundary_reference_cpu_seconds'] = median
    for multiplier in [.95, 1, 1.05]:
        payload = {'mode': 0, 'iterations': 100_000_000, 'samples': args.samples,
                   'cpu_budget': median * multiplier}
        for client in clients:
            report['measurements'].append(record(f'boundary-{multiplier}-sequential', 'cpu', client, payload))
            save()
        with ThreadPoolExecutor(max_workers=len(clients)) as pool:
            report['measurements'].extend(pool.map(
                lambda c: record(f'boundary-{multiplier}-parallel', 'cpu', c, payload), clients))
        save()
        print(args.target, 'boundary', multiplier, 'completed', flush=True)

    for i in range(20):
        report['measurements'].append(record('latency', 'cpu', clients[i % len(clients)],
            {'mode': 0, 'iterations': 100_000_000, 'samples': 1, 'compile': not args.precompiled}))
        save()
    report['measurements'].append(record('watchdog', 'cpu', clients[0],
        {'mode': 0, 'iterations': 300_000_000, 'wall_seconds': .01}))
    report['measurements'].append(record('abort', 'abort', clients[0], {'mode': 3, 'iterations': 1}))
    if args.target == 'microvm':
        s3 = boto3.client('s3', region_name='ap-northeast-1')
        report['cold_launches'] = [json.loads(s3.get_object(Bucket=outputs['Bucket'],
            Key=f'launch/microvm-{i}.json')['Body'].read()) for i in range(3)]
    save()
    print('Saved', args.report, flush=True)


if __name__ == '__main__':
    main()
