import { execSync } from 'node:child_process'
import { App, Aspects, CfnOutput, CfnResource, Duration, RemovalPolicy, Stack,
  aws_autoscaling as asg, aws_ec2 as ec2, aws_iam as iam, aws_s3 as s3,
  aws_s3_assets as assets, aws_sqs as sqs } from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Use a codex- branch')
const stack = new Stack(app, `${branch.replace(/[^A-Za-z0-9-]/g, '-')}-FairnessEC2`, { env: { region: 'ap-northeast-1' } })
const dlq = new sqs.Queue(stack, 'Deadletters', { retentionPeriod: Duration.days(14) })
const queue = new sqs.Queue(stack, 'Submissions', { visibilityTimeout: Duration.seconds(60),
  deadLetterQueue: { queue: dlq, maxReceiveCount: 3 } })
const bucket = new s3.Bucket(stack, 'Evidence', { versioned: true, enforceSSL: true,
  blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY })
const asset = new assets.Asset(stack, 'Host', { path: '../work/fairness-host' })
const vpc = new ec2.Vpc(stack, 'Execution', { maxAzs: 1, natGateways: 0,
  subnetConfiguration: [{ name: 'Public', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 }] })
const sg = new ec2.SecurityGroup(stack, 'Network', { vpc, allowAllOutbound: false })
sg.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443))
const role = new iam.Role(stack, 'HostRole', { assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com') })
asset.grantRead(role); bucket.grantReadWrite(role); queue.grantConsumeMessages(role)
const data = ec2.UserData.forLinux()
data.addCommands('set -euxo pipefail', 'dnf install -y docker python3-pip', 'systemctl enable --now docker',
  'mkdir -p /opt/fairness', `aws s3 cp s3://${asset.s3BucketName}/${asset.s3ObjectKey} /opt/fairness/source.zip --region ap-northeast-1`,
  'python3 -m zipfile -e /opt/fairness/source.zip /opt/fairness',
  'cd /opt/fairness && sha512sum -c gvisor.sha512 && tar -xjf gvisor.tar.bz2 -C /usr/local/bin',
  'chmod -R a+rX /usr/local/bin/gvisor-bin', 'runsc install -- --platform=systrap', 'systemctl restart docker',
  'cd /opt/fairness/image && docker build -t fairness:proof .',
  'python3 -m venv /opt/fairness/venv', '/opt/fairness/venv/bin/pip install boto3==1.42.20',
  "cat > /etc/systemd/system/fairness.service <<'UNIT'", '[Unit]', 'After=docker.service network-online.target', '[Service]',
  `Environment=BUCKET=${bucket.bucketName}`, `Environment=QUEUE=${queue.queueUrl}`, 'Environment=AWS_DEFAULT_REGION=ap-northeast-1',
  'ExecStart=/opt/fairness/venv/bin/python /opt/fairness/agent.py', 'Restart=always', 'RestartSec=2',
  '[Install]', 'WantedBy=multi-user.target', 'UNIT', 'systemctl daemon-reload', 'systemctl enable --now fairness.service')
const template = new ec2.LaunchTemplate(stack, 'Template', { role, securityGroup: sg, userData: data,
  instanceType: new ec2.InstanceType('c7g.large'), machineImage: ec2.MachineImage.latestAmazonLinux2023({
    cpuType: ec2.AmazonLinuxCpuType.ARM_64 }), requireImdsv2: true, httpPutResponseHopLimit: 1,
  blockDevices: [{ deviceName: '/dev/xvda', volume: ec2.BlockDeviceVolume.ebs(12, { encrypted: true, deleteOnTermination: true }) }] })
const pool = new asg.AutoScalingGroup(stack, 'Pool', { vpc, launchTemplate: template, minCapacity: 0,
  maxCapacity: 2, desiredCapacity: 2, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC } })
for (const [key, value] of Object.entries({ Queue: queue.queueUrl, DLQ: dlq.queueUrl,
  Bucket: bucket.bucketName, ASG: pool.autoScalingGroupName })) new CfnOutput(stack, key, { value })
Aspects.of(app).add({ visit(node) { if (node instanceof CfnResource) node.applyRemovalPolicy(RemovalPolicy.DESTROY) } })
