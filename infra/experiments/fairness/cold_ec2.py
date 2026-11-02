"""Best-case manual wake of the CFN-owned ASG after confirming zero EC2 instances."""
import json
from pathlib import Path
import time

import boto3
from botocore.parsers import ResponseParserError
from botocore.exceptions import EndpointConnectionError, ReadTimeoutError, ConnectionClosedError

from run import Queued


def call(function, **kwargs):
    # These are reads or idempotent fixed-capacity writes. Preserve the experiment timer.
    for attempt in range(5):
        try:
            return function(**kwargs)
        except (ResponseParserError, EndpointConnectionError, ReadTimeoutError, ConnectionClosedError):
            if attempt == 4:
                raise
            time.sleep(.5)


def main():
    root = Path(__file__).resolve().parents[3]
    out = next(iter(json.loads((root / 'work/fairness-ec2-outputs.json').read_text()).values()))
    asg = boto3.client('autoscaling', region_name='ap-northeast-1')
    ec2 = boto3.client('ec2', region_name='ap-northeast-1')
    sqs = boto3.client('sqs', region_name='ap-northeast-1')
    group = call(asg.describe_auto_scaling_groups, AutoScalingGroupNames=[out['ASG']])['AutoScalingGroups'][0]
    old = [v['InstanceId'] for v in group['Instances']]
    for _ in range(30):
        values = sqs.get_queue_attributes(QueueUrl=out['Queue'], AttributeNames=[
            'ApproximateNumberOfMessages', 'ApproximateNumberOfMessagesNotVisible'])['Attributes']
        if all(int(v) == 0 for v in values.values()):
            break
        time.sleep(1)
    else:
        raise RuntimeError('Queue is not idle; refusing scale-in')
    call(asg.set_desired_capacity, AutoScalingGroupName=out['ASG'], DesiredCapacity=0)
    start = time.monotonic()
    while time.monotonic() - start < 240:
        group = call(asg.describe_auto_scaling_groups, AutoScalingGroupNames=[out['ASG']])['AutoScalingGroups'][0]
        scoped = call(ec2.describe_instances, Filters=[{'Name': 'tag:aws:autoscaling:groupName', 'Values': [out['ASG']]},
            {'Name': 'instance-state-name', 'Values': ['pending', 'running', 'stopping', 'stopped', 'shutting-down']}])
        states = [i['State']['Name'] for r in scoped['Reservations'] for i in r['Instances']]
        if not group['Instances'] and not states:
            break
        time.sleep(2)
    else:
        raise TimeoutError('EC2 did not reach zero')
    report = {'zero_confirmed': True, 'initial_members': len(old), 'nonterminated_instances_at_zero': len(states),
        'wake_policy': 'Manual SetDesiredCapacity(1) immediately after SQS accepted the message.',
        'limitation': 'This AMI bootstraps packages and the image. Not a prepared-AMI benchmark.'}
    path = root / 'work/fairness-cold-ec2.json'
    path.write_text(json.dumps(report, indent=2) + '\n')
    print('ASG membership and EC2 running count both zero', flush=True)
    result = Queued(out, 0).run({'compile': True, 'samples': 1, 'iterations': 100_000_000},
        on_enqueued=lambda: call(asg.set_desired_capacity, AutoScalingGroupName=out['ASG'], DesiredCapacity=1))
    report['result'] = result
    path.write_text(json.dumps(report, indent=2) + '\n')
    print('EC2 zero -> first result', result['client_seconds'], flush=True)


if __name__ == '__main__':
    main()
