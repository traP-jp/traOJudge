import { execSync } from 'node:child_process'
import { App, Aspects, CfnOutput, CfnResource, Duration, RemovalPolicy, Stack,
  aws_autoscaling as asg, aws_ec2 as ec2, aws_iam as iam, aws_rds as rds,
  aws_s3 as s3, aws_s3_assets as assets, aws_lambda as lambda,
  aws_events as events, aws_events_targets as targets, aws_logs as logs } from 'aws-cdk-lib'

// Disposable distributed-workload proof; no production rollout or public API.
const app = new App({ analyticsReporting: false })
const hardened = String(app.node.tryGetContext('hardened') ?? 'false') === 'true'
const generation = String(app.node.tryGetContext('generation') ?? '')
if (!/^[A-Za-z0-9-]*$/.test(generation)) throw new Error('Invalid proof generation')
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Use a codex- branch')
const stack = new Stack(app, `${branch.replace(/[^A-Za-z0-9-]/g, '-')}-PostgresPool${generation}`, { env: { region: 'ap-northeast-1' } })
const vpc = new ec2.Vpc(stack, 'Network', { maxAzs: 2, natGateways: 0,
  subnetConfiguration: [{ name: 'Workers', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 },
    { name: 'Database', subnetType: ec2.SubnetType.PRIVATE_ISOLATED, cidrMask: 24 }] })
const hostSg = new ec2.SecurityGroup(stack, 'Workers', { vpc, allowAllOutbound: false })
hostSg.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443))
const dbSg = new ec2.SecurityGroup(stack, 'DatabaseFirewall', { vpc, allowAllOutbound: false })
const database = new rds.DatabaseInstance(stack, 'Database', { vpc, securityGroups: [dbSg],
  vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_ISOLATED },
  engine: rds.DatabaseInstanceEngine.postgres({ version: rds.PostgresEngineVersion.of('17.11', '17') }),
  instanceType: new ec2.InstanceType('t4g.micro'), databaseName: 'judge',
  credentials: rds.Credentials.fromGeneratedSecret('probe'), allocatedStorage: 20,
  storageType: rds.StorageType.GP3, storageEncrypted: true, multiAz: hardened,
  backupRetention: Duration.days(hardened ? 7 : 0), deleteAutomatedBackups: true, deletionProtection: false,
  publiclyAccessible: false, removalPolicy: RemovalPolicy.DESTROY })
database.connections.allowDefaultPortFrom(hostSg)
const asset = new assets.Asset(stack, 'PreparedWorkload', { path: '../work/pg-pool-host', exclude: ['images.tar.gz'] })
const imageAsset = new assets.Asset(stack, 'PreparedImage', { path: '../work/pg-pool-host/images.tar.gz' })
const bucket = new s3.Bucket(stack, 'Evidence', { versioned: true, enforceSSL: true,
  blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY })
const role = new iam.Role(stack, 'HostRole', { assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
  managedPolicies: [iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMManagedInstanceCore')] })
asset.grantRead(role); imageAsset.grantRead(role); bucket.grantReadWrite(role); database.secret!.grantRead(role)
const data = ec2.UserData.forLinux()
data.addCommands('set -euxo pipefail', 'dnf install -y docker', 'systemctl enable --now docker',
  'mkdir -p /opt/judge-pool /var/lib/judge-probe/jobs',
  `aws s3 cp s3://${asset.s3BucketName}/${asset.s3ObjectKey} /opt/judge-pool/source.zip --region ap-northeast-1`,
  'python3 -m zipfile -e /opt/judge-pool/source.zip /opt/judge-pool',
  `aws s3 cp s3://${imageAsset.s3BucketName}/${imageAsset.s3ObjectKey} /opt/judge-pool/images.tar.gz --region ap-northeast-1`,
  'cd /opt/judge-pool && sha512sum -c gvisor.sha512 && tar -xjf gvisor.tar.bz2 -C /usr/local/bin',
  'chmod -R a+rX /usr/local/bin/gvisor-bin', 'runsc install -- --platform=systrap', 'systemctl restart docker',
  'docker load -i /opt/judge-pool/images.tar.gz',
  'chmod 755 /opt/judge-pool/judge-probe',
  'docker create --name judge-template codex-judge-pool:proof',
  'docker cp judge-template:/nix /nix', 'docker rm judge-template',
  'set +x', 'TOKEN=$(curl -fsS -X PUT -H "X-aws-ec2-metadata-token-ttl-seconds: 300" http://169.254.169.254/latest/api/token)',
  'export INSTANCE=$(curl -fsS -H "X-aws-ec2-metadata-token: $TOKEN" http://169.254.169.254/latest/meta-data/instance-id)',
  'set +x',
  `aws secretsmanager get-secret-value --secret-id '${database.secret!.secretArn}' --query SecretString --output text --region ap-northeast-1 > /opt/judge-pool/secret.json`,
  "python3 - <<'PY'", 'import json,pathlib,urllib.parse,os',
  "x=json.loads(pathlib.Path('/opt/judge-pool/secret.json').read_text())",
  `host='${database.dbInstanceEndpointAddress}'`,
  "url='postgresql://'+urllib.parse.quote(x['username'],safe='')+':'+urllib.parse.quote(x['password'],safe='')+'@'+host+':5432/judge?sslmode=verify-full&sslrootcert=/opt/rds-ca.pem'",
  "pathlib.Path('/opt/judge-pool/control.env').write_text('DATABASE_URL='+url+'\\nJUDGE_WORKER='+os.environ['INSTANCE']+'\\n')",
  "os.chmod('/opt/judge-pool/control.env',0o600)", 'PY',
  'rm /opt/judge-pool/secret.json', 'set -x',
  "cat > /etc/systemd/system/judge-watchdog.service <<'UNIT'", '[Unit]', 'After=docker.service',
  '[Service]', 'ExecStart=/usr/bin/python3 /opt/judge-pool/watchdog.py', 'Restart=always', 'RestartSec=1',
  '[Install]', 'WantedBy=multi-user.target', 'UNIT',
  "cat > /etc/systemd/system/judge-worker.service <<'UNIT'", '[Unit]',
  'After=docker.service judge-watchdog.service', 'Requires=judge-watchdog.service', '[Service]',
  'ExecStartPre=/usr/bin/python3 /opt/judge-pool/watchdog.py --all',
  'ExecStartPre=-/usr/bin/docker rm -f judge-control',
  'ExecStart=/usr/bin/docker run --rm --name judge-control --network=host --cgroupns=host --cpuset-cpus=0 --env-file /opt/judge-pool/control.env -v /opt/judge-pool/rds-ca.pem:/opt/rds-ca.pem:ro -v /opt/judge-pool/judge-probe:/opt/judge-probe:ro -v /var/run/docker.sock:/var/run/docker.sock -v /sys/fs/cgroup:/host-cgroup:ro -v /proc:/host-proc:ro -v /nix:/nix -v /var/lib/judge-probe/jobs:/var/lib/judge-probe/jobs --entrypoint /opt/tools/bin/python3 codex-judge-pool:proof /opt/supervisor.py',
  'ExecStop=/usr/bin/docker stop -t 2 judge-control', 'Restart=always', 'RestartSec=1',
  '[Install]', 'WantedBy=multi-user.target', 'UNIT',
  'systemctl daemon-reload', 'systemctl enable --now judge-watchdog.service judge-worker.service')
const userData = data
const template = new ec2.LaunchTemplate(stack, 'Template', { role, securityGroup: hostSg, userData,
  instanceType: new ec2.InstanceType('c7i.xlarge'), requireImdsv2: true, httpPutResponseHopLimit: 1,
  machineImage: ec2.MachineImage.latestAmazonLinux2023({ cpuType: ec2.AmazonLinuxCpuType.X86_64 }),
  blockDevices: [{ deviceName: '/dev/xvda', volume: ec2.BlockDeviceVolume.ebs(24, { encrypted: true, deleteOnTermination: true }) }] })
const launch = template.node.defaultChild as ec2.CfnLaunchTemplate
launch.addPropertyOverride('LaunchTemplateData.CpuOptions', { CoreCount: 2, ThreadsPerCore: 1 })
const pool = new asg.AutoScalingGroup(stack, 'Pool', { vpc, launchTemplate: template,
  minCapacity: 0, maxCapacity: 5, desiredCapacity: 0,
  vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC } })
pool.node.addDependency(database)
const controllerSg = new ec2.SecurityGroup(stack, 'ControllerFirewall', { vpc, allowAllOutbound: false })
database.connections.allowDefaultPortFrom(controllerSg)
const endpoint = vpc.addInterfaceEndpoint('AutoscalingAPI', {
  service: ec2.InterfaceVpcEndpointAwsService.AUTOSCALING, open: false,
  subnets: { subnets: [vpc.isolatedSubnets[0]] } })
endpoint.connections.allowDefaultPortFrom(controllerSg)
const controller = new lambda.Function(stack, 'Controller', { runtime: lambda.Runtime.PYTHON_3_13,
  architecture: lambda.Architecture.X86_64, handler: 'control.handler',
  code: lambda.Code.fromAsset('../work/pg-pool-control'), memorySize: 512, timeout: Duration.seconds(60),
  logGroup: new logs.LogGroup(stack,'ControllerLogs',{ retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY }),
  vpc, vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_ISOLATED }, securityGroups: [controllerSg],
  environment: { DB_HOST: database.dbInstanceEndpointAddress, DB_USER: 'probe',
    DB_PASSWORD: database.secret!.secretValueFromJson('password').unsafeUnwrap(), ASG: pool.autoScalingGroupName } })
controller.addToRolePolicy(new iam.PolicyStatement({ actions: ['autoscaling:DescribeAutoScalingGroups'], resources: ['*'] }))
controller.addToRolePolicy(new iam.PolicyStatement({ actions: ['autoscaling:SetDesiredCapacity'], resources: [pool.autoScalingGroupArn] }))
new events.Rule(stack, 'RecoveryTick', { schedule: events.Schedule.rate(Duration.minutes(1)),
  targets: [new targets.LambdaFunction(controller, { event: events.RuleTargetInput.fromObject({ action: 'tick' }) })] })
for (const [key,value] of Object.entries({ ASG: pool.autoScalingGroupName, Bucket: bucket.bucketName,
  ControllerName: controller.functionName })) {
  new CfnOutput(stack,key,{value})
}
Aspects.of(app).add({ visit(node) { if (node instanceof CfnResource) node.applyRemovalPolicy(RemovalPolicy.DESTROY) } })
