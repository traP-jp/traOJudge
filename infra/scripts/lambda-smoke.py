"""CDK deploy -> two synchronous invocations -> CDK destroy -> absence checks.

Run from infra/ after sourcing /workspace/tooling/env.sh and selecting the AWS profile.
Only public, sanitized evidence is written to the requested report path.
"""
import argparse
import datetime
import json
from pathlib import Path
import subprocess
import time

import boto3
from botocore.exceptions import ClientError


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default='../work/lambda-smoke-rerun.json')
    args = parser.parse_args()
    branch = subprocess.check_output(['git', 'branch', '--show-current'], text=True).strip()
    if not branch.startswith('codex-'):
        raise ValueError('Check out a codex-<topic> branch first.')
    stack = ''.join(c if c.isascii() and (c.isalnum() or c == '-') else '-' for c in branch) + '-LambdaSmoke'
    cdk = ['npx', 'aws-cdk']
    selection = ['--app', 'npx tsx bin/lambda-smoke.ts', '--output', '../work/cdk-lambda-smoke']
    cf = boto3.client('cloudformation', region_name='ap-northeast-1')
    fn = boto3.client('lambda', region_name='ap-northeast-1')
    outputs = Path('../work/lambda-smoke-outputs.json')
    report = {'branch': branch, 'region': 'ap-northeast-1',
              'at': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'invocations': []}
    resources = []
    try:
        # Remove an earlier run of this branch's smoke stack, per infra/PATTERNS.md.
        try:
            cf.describe_stacks(StackName=stack)
        except ClientError as exc:
            if exc.response['Error']['Code'] != 'ValidationError' or 'does not exist' not in str(exc):
                raise
        else:
            subprocess.run(cdk + ['destroy', *selection, '--force'], check=True)
        subprocess.run(cdk + ['deploy', *selection, '--require-approval', 'never',
                             '--outputs-file', str(outputs)], check=True)
        name = json.loads(outputs.read_text())[stack]['FunctionName']
        resources = cf.describe_stack_resources(StackName=stack)['StackResources']
        report['creation'] = cf.describe_stacks(StackName=stack)['Stacks'][0]['StackStatus']
        report['resource_types'] = sorted(r['ResourceType'] for r in resources)
        for payload in [{'message': 'first', 'number': 1},
                        {'message': '二つ目', 'number': 2, 'nested': {'changed': True}}]:
            started = time.monotonic()
            response = fn.invoke(FunctionName=name, InvocationType='RequestResponse',
                                 Payload=json.dumps(payload, ensure_ascii=False).encode())
            result = json.loads(response['Payload'].read())
            if response.get('FunctionError') or result != {'ok': True, 'received': payload}:
                raise RuntimeError('Invocation failed or response differs from the request.')
            report['invocations'].append({'request': payload, 'response': result,
                'status_code': response['StatusCode'], 'passed': True,
                'client_seconds': round(time.monotonic() - started, 4)})
    finally:
        subprocess.run(cdk + ['destroy', *selection, '--force'], check=True)
        report['deletion'] = verify_deleted(cf, fn, stack, resources)
        Path(args.report).write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps(report, ensure_ascii=False))


def verify_deleted(cf, fn, stack, resources):
    checks = {}
    try:
        cf.describe_stacks(StackName=stack)
    except ClientError as exc:
        if exc.response['Error']['Code'] != 'ValidationError' or 'does not exist' not in str(exc):
            raise
        checks['stack_absent'] = True
    else:
        raise RuntimeError('Stack remains after destroy.')
    for resource in resources:
        kind, physical = resource['ResourceType'], resource['PhysicalResourceId']
        if kind == 'AWS::Lambda::Function':
            try:
                fn.get_function(FunctionName=physical)
            except fn.exceptions.ResourceNotFoundException:
                checks['function_absent'] = True
            else:
                raise RuntimeError('Function remains after destroy.')
        elif kind == 'AWS::IAM::Role':
            iam = boto3.client('iam')
            try:
                iam.get_role(RoleName=physical)
            except iam.exceptions.NoSuchEntityException:
                checks['role_absent'] = True
            else:
                raise RuntimeError('Role remains after destroy.')
        elif kind == 'AWS::Logs::LogGroup':
            logs = boto3.client('logs', region_name='ap-northeast-1')
            groups = logs.describe_log_groups(logGroupNamePrefix=physical)['logGroups']
            checks['logs_absent'] = not any(g['logGroupName'] == physical for g in groups)
            if not checks['logs_absent']:
                raise RuntimeError('Logs remain after destroy.')
    return checks


if __name__ == '__main__':
    main()
