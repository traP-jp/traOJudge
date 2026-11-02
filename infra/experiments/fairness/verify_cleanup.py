"""Verify absence of this experiment's stacks and compute resources without exposing IDs."""
import json
import datetime
from pathlib import Path

import boto3
from botocore.exceptions import ClientError


def absent(function, missing, **kwargs):
    try:
        function(**kwargs)
    except ClientError as exc:
        if exc.response['Error']['Code'] in missing:
            return True
        raise
    return False


def main():
    root = Path(__file__).resolve().parents[3]
    cf = boto3.client('cloudformation', region_name='ap-northeast-1')
    fn = boto3.client('lambda', region_name='ap-northeast-1')
    vm = boto3.client('lambda-microvms', region_name='ap-northeast-1')
    ec2 = boto3.client('ec2', region_name='ap-northeast-1')
    path = root / 'work/fairness-cleanup.json'
    report = json.loads(path.read_text())
    report['checks'] = {}
    for suffix in ['FairnessQueue', 'Fairness', 'FairnessEC2']:
        name = 'codex-judge-fairness-' + suffix
        try:
            cf.describe_stacks(StackName=name)
        except ClientError as exc:
            if exc.response['Error']['Code'] != 'ValidationError' or 'does not exist' not in str(exc):
                raise
        else:
            raise RuntimeError(name + ' still exists')
        checks = {'stack_absent': True, 'functions_absent': 0, 'vpcs_absent': 0, 'microvms_off': 0}
        for resource in json.loads((root / 'work' / (suffix + '-resources.json')).read_text()):
            kind, physical = resource['ResourceType'], resource.get('PhysicalResourceId')
            if not physical:
                continue
            if kind == 'AWS::Lambda::Function':
                if not absent(fn.get_function, ['ResourceNotFoundException'], FunctionName=physical):
                    raise RuntimeError('A function remains')
                checks['functions_absent'] += 1
            if kind == 'AWS::EC2::VPC':
                if not absent(ec2.describe_vpcs, ['InvalidVpcID.NotFound'], VpcIds=[physical]):
                    raise RuntimeError('A VPC remains')
                checks['vpcs_absent'] += 1
            if kind == 'AWS::CloudFormation::CustomResource' and physical.startswith('microvm-'):
                try:
                    state = vm.get_microvm(microvmIdentifier=physical)['state']
                except vm.exceptions.ResourceNotFoundException:
                    state = 'DELETED'
                if state not in ['TERMINATED', 'DELETED']:
                    raise RuntimeError('A MicroVM remains active')
                checks['microvms_off'] += 1
            if kind == 'AWS::Lambda::MicrovmImage':
                if not absent(vm.get_microvm_image, ['ResourceNotFoundException'], imageIdentifier=physical):
                    raise RuntimeError('A MicroVM image remains')
                checks['microvm_image_absent'] = True
        report['checks'][name] = checks
    live = ec2.describe_instances(Filters=[{'Name': 'tag:aws:cloudformation:stack-name',
        'Values': ['codex-judge-fairness-FairnessEC2']}, {'Name': 'instance-state-name',
        'Values': ['pending', 'running', 'stopped', 'stopping', 'shutting-down']}])
    report['nonterminated_ec2_instances'] = sum(len(r['Instances']) for r in live['Reservations'])
    if report['nonterminated_ec2_instances']:
        raise RuntimeError('An EC2 instance remains')
    report['shared_bootstrap_retained'] = cf.describe_stacks(StackName='CDKToolkit')['Stacks'][0]['StackStatus'] == 'CREATE_COMPLETE'
    report['verified_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    early = root / 'work/fairness-live-debug.json'
    if early.exists():
        report['early_failed_creation_microvms_off'] = 0
        for item in json.loads(early.read_text())['items']:
            try:
                state = vm.get_microvm(microvmIdentifier=item['microvmId'])['state']
            except vm.exceptions.ResourceNotFoundException:
                state = 'DELETED'
            if state not in ['TERMINATED', 'DELETED']:
                raise RuntimeError('A VM from an earlier failed creation remains')
            report['early_failed_creation_microvms_off'] += 1
    (root / 'docs/services/fairness-teardown.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report['checks']))


if __name__ == '__main__':
    main()
