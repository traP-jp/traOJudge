import { execFileSync, execSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import {
  App, ArnFormat, Aspects, CfnOutput, CfnResource, DockerBuildSecret, Duration, RemovalPolicy, Stack,
  aws_cloudwatch as cloudwatch, aws_cloudwatch_actions as alarmActions,
  aws_autoscaling as autoscaling, aws_dynamodb as dynamodb, aws_ec2 as ec2,
  aws_ecr_assets as images, aws_events as events, aws_events_targets as targets,
  aws_iam as iam, aws_lambda as lambda, aws_lambda_event_sources as sources,
  aws_logs as logs, aws_s3 as s3, aws_s3_assets as assets, aws_sqs as sqs,
} from 'aws-cdk-lib'

export function buildPool(app: App) {
  const branch = execSync('git branch --show-current').toString().trim()
  if (!branch.startsWith('codex-')) throw new Error('Use a codex-<topic> branch.')
  const prefix = branch.replace(/[^A-Za-z0-9-]/g, '-')
  const proofName = app.node.tryGetContext('proofName') ?? 'Pool'
  if (!/^[A-Za-z][A-Za-z0-9-]*$/.test(proofName)) throw new Error('Invalid proofName.')
  const stack = new Stack(app, `${prefix}-${proofName}`, { env: { region: 'ap-northeast-1' },
    description: 'Warm native judge pool with fresh offline gVisor sandboxes; scales to zero between sessions.' })
  const root = process.cwd()
  for (const obsolete of ['poolMax', 'nodeVisibility', 'nodeAttempts', 'idleSeconds']) {
    if (app.node.tryGetContext(obsolete) !== undefined) throw new Error(`Use a named poolPolicy/poolProfile, not ${obsolete}.`)
  }
  const policyPath = path.resolve(root, app.node.tryGetContext('poolPolicy') ?? 'policy/pool.json')
  const policyProfile = app.node.tryGetContext('poolProfile') ?? 'lookahead'
  const policyJson = execFileSync('python3', [path.join(root, 'runtime/policy.py'), policyPath, policyProfile], { encoding: 'utf8' }).trim()
  const policy = JSON.parse(policyJson)
  const maximum: number = policy.limits.max_instances
  const visibility: number = policy.queue.visibility_seconds
  const attempts: number = policy.queue.max_receive_count
  const certificate = process.env.CODEX_PROXY_CERT
  const secrets: Record<string, string> = certificate ? { proxy_ca: DockerBuildSecret.fromSrc(certificate) } : {}
  const image = new images.DockerImageAsset(stack, 'WorkerImage', {
    directory: path.join(root, 'worker'), buildSecrets: secrets, platform: images.Platform.LINUX_AMD64,
  })
  const code = lambda.DockerImageCode.fromImageAsset(path.join(root, 'runtime'), {
    buildSecrets: secrets, platform: images.Platform.LINUX_AMD64,
  })
  const agent = new assets.Asset(stack, 'HostAgent', { path: path.join(root, 'pool'), exclude: ['__pycache__'] })
  const checksum: string = JSON.parse(readFileSync(path.join(root, 'pool/gvisor-version.json'), 'utf8')).sha512
  const bucket = new s3.Bucket(stack, 'Files', { versioned: true, enforceSSL: true,
    blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY })
  const table = new dynamodb.Table(stack, 'Acceptance', {
    partitionKey: { name: 'pk', type: dynamodb.AttributeType.STRING }, billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
    stream: dynamodb.StreamViewType.NEW_IMAGE, removalPolicy: RemovalPolicy.DESTROY,
  })
  const controlDlq = new sqs.Queue(stack, 'ControlDLQ', { fifo: true, retentionPeriod: Duration.days(14) })
  const nodeDlq = new sqs.Queue(stack, 'NodeDLQ', { retentionPeriod: Duration.days(14) })
  const controlQueue = new sqs.Queue(stack, 'ControlQueue', { fifo: true, visibilityTimeout: Duration.minutes(3),
    deadLetterQueue: { queue: controlDlq, maxReceiveCount: 5 } })
  const nodeQueue = new sqs.Queue(stack, 'NodeQueue', { visibilityTimeout: Duration.seconds(visibility),
    retentionPeriod: Duration.days(4), deadLetterQueue: { queue: nodeDlq, maxReceiveCount: attempts } })
  const vpc = new ec2.Vpc(stack, 'ExecutionVPC', { natGateways: 0, maxAzs: policy.placement.availability_zone_count,
    subnetConfiguration: [{ name: 'Execution', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 }] })
  const network = new ec2.SecurityGroup(stack, 'HostNetwork', { vpc, allowAllOutbound: false })
  network.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443), 'Host HTTPS; guest has no network')
  const role = new iam.Role(stack, 'HostRole', { assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com') })
  agent.grantRead(role)
  image.repository.grantPull(role)
  bucket.grantRead(role, 'inputs/*')
  for (const prefix of ['artifacts', 'results', 'workers']) bucket.grantReadWrite(role, `${prefix}/*`)
  nodeQueue.grantConsumeMessages(role)
  controlQueue.grantSendMessages(role)
  role.addToPolicy(new iam.PolicyStatement({ actions: ['autoscaling:DescribeAutoScalingInstances'], resources: ['*'] }))
  role.addToPolicy(new iam.PolicyStatement({ actions: ['autoscaling:SetInstanceProtection'],
    resources: [stack.formatArn({ service: 'autoscaling', resource: 'autoScalingGroup',
      resourceName: `*:autoScalingGroupName/${stack.stackName}-*`, arnFormat: ArnFormat.COLON_RESOURCE_NAME })] }))
  const userData = ec2.UserData.forLinux()
  userData.addCommands('set -euxo pipefail',
    'dnf install -y docker python3-pip unzip tar bzip2',
    'systemctl enable --now docker',
    'install -d -m 755 /opt/traojudge',
    `aws s3 cp s3://${agent.s3BucketName}/${agent.s3ObjectKey} /opt/traojudge/source.zip --region ap-northeast-1`,
    'python3 -m zipfile -e /opt/traojudge/source.zip /opt/traojudge',
    `printf '%s  %s\n' '${checksum}' '/opt/traojudge/gvisor.tar.bz2' | sha512sum -c -`,
    'tar -xjf /opt/traojudge/gvisor.tar.bz2 -C /usr/local/bin',
    'chmod 755 /usr/local/bin/runsc',
    'chmod -R a+rX /usr/local/bin/gvisor-bin',
    '/usr/local/bin/runsc install -- --platform=systrap',
    'systemctl restart docker',
    'python3 -m venv /opt/traojudge/venv',
    '/opt/traojudge/venv/bin/pip install boto3==1.42.20',
    `aws ecr get-login-password --region ap-northeast-1 | docker login --username AWS --password-stdin ${stack.account}.dkr.ecr.${stack.region}.amazonaws.com`,
    `docker pull ${image.imageUri}`,
    "cat > /etc/systemd/system/traojudge.service <<'SERVICE'",
    '[Unit]', 'After=docker.service network-online.target', 'Requires=docker.service', '[Service]',
    'Type=simple', `Environment=BUCKET=${bucket.bucketName}`, `Environment=NODE_QUEUE=${nodeQueue.queueUrl}`,
    `Environment=CONTROL_QUEUE=${controlQueue.queueUrl}`, `Environment=IMAGE=${image.imageUri}`,
    `Environment='POOL_POLICY=${policyJson}'`,
    'Environment=AWS_DEFAULT_REGION=ap-northeast-1',
    'ExecStart=/opt/traojudge/venv/bin/python /opt/traojudge/agent.py',
    'Restart=always', 'RestartSec=2', 'StandardOutput=journal+console', 'StandardError=journal+console',
    '[Install]', 'WantedBy=multi-user.target', 'SERVICE',
    'systemctl daemon-reload', 'systemctl enable --now traojudge.service')
  const template = new ec2.LaunchTemplate(stack, 'HostTemplate', { role, securityGroup: network, userData,
    instanceType: new ec2.InstanceType('c7i.large'), machineImage: ec2.MachineImage.latestAmazonLinux2023(),
    requireImdsv2: true, httpPutResponseHopLimit: 1,
    blockDevices: [{ deviceName: '/dev/xvda', volume: ec2.BlockDeviceVolume.ebs(12, { encrypted: true, deleteOnTermination: true }) }],
  })
  const pool = new autoscaling.AutoScalingGroup(stack, 'Pool', { vpc, launchTemplate: template,
    vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC }, minCapacity: 0, maxCapacity: maximum, desiredCapacity: policy.capacity.warm_instances,
    newInstancesProtectedFromScaleIn: true,
    minHealthyPercentage: policy.placement.replacement_min_healthy_percentage,
    maxHealthyPercentage: policy.placement.replacement_max_healthy_percentage })
  const namespace = `traOJudge/${stack.stackName}`
  const desiredMetric = new cloudwatch.Metric({ namespace, metricName: 'SafeDesiredInstances',
    statistic: 'Maximum', period: Duration.seconds(policy.scaling.metric_period_seconds) })
  const upper = new autoscaling.StepScalingAction(stack, 'NonzeroDesired', { autoScalingGroup: pool,
    adjustmentType: autoscaling.AdjustmentType.EXACT_CAPACITY, estimatedInstanceWarmup: Duration.seconds(0),
    metricAggregationType: autoscaling.MetricAggregationType.MAXIMUM })
  for (let n = 1; n <= maximum; n++) upper.addAdjustment({ adjustment: n, lowerBound: n - 1,
    ...(n < maximum ? { upperBound: n } : {}) })
  const zero = new autoscaling.StepScalingAction(stack, 'ZeroDesired', { autoScalingGroup: pool,
    adjustmentType: autoscaling.AdjustmentType.EXACT_CAPACITY, estimatedInstanceWarmup: Duration.seconds(0) })
  zero.addAdjustment({ adjustment: 0, upperBound: 0 })
  const upperAlarm = new cloudwatch.Alarm(stack, 'NonzeroDemandAlarm', { metric: desiredMetric, threshold: 0.5,
    comparisonOperator: cloudwatch.ComparisonOperator.GREATER_THAN_THRESHOLD, evaluationPeriods: 1,
    treatMissingData: cloudwatch.TreatMissingData.NOT_BREACHING })
  const zeroAlarm = new cloudwatch.Alarm(stack, 'ZeroDemandAlarm', { metric: desiredMetric, threshold: 0.5,
    comparisonOperator: cloudwatch.ComparisonOperator.LESS_THAN_THRESHOLD, evaluationPeriods: 1,
    treatMissingData: cloudwatch.TreatMissingData.NOT_BREACHING })
  upperAlarm.addAlarmAction(new alarmActions.AutoScalingAction(upper))
  zeroAlarm.addAlarmAction(new alarmActions.AutoScalingAction(zero))
  const env = { POOL_POLICY: policyJson, BUCKET: bucket.bucketName, TABLE: table.tableName, CONTROL_QUEUE: controlQueue.queueUrl, NODE_QUEUE: nodeQueue.queueUrl }
  function fn(id: string, mode: string, extra: Record<string, string> = {}, reservedConcurrentExecutions?: number) {
    const logGroup = new logs.LogGroup(stack, `${id}Logs`, { retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY })
    return new lambda.DockerImageFunction(stack, id, { code, memorySize: 1024, timeout: Duration.seconds(30), logGroup,
      environment: { ...env, MODE: mode, ...extra }, reservedConcurrentExecutions })
  }
  const capacity = fn('Capacity', 'capacity', { ASG: pool.autoScalingGroupName,
    METRIC_NAMESPACE: namespace })
  table.grantReadWriteData(capacity)
  bucket.grantReadWrite(capacity, 'workers/*')
  controlQueue.grantConsumeMessages(capacity)
  nodeQueue.grantConsumeMessages(capacity)
  capacity.addToRolePolicy(new iam.PolicyStatement({ actions: ['cloudwatch:PutMetricData'], resources: ['*'],
    conditions: { StringEquals: { 'cloudwatch:namespace': namespace } } }))
  capacity.addToRolePolicy(new iam.PolicyStatement({ actions: ['autoscaling:DescribeAutoScalingGroups'], resources: ['*'] }))
  new events.Rule(stack, 'ReconcileCapacity', { schedule: events.Schedule.rate(Duration.minutes(1)), targets: [new targets.LambdaFunction(capacity)] })
  const api = fn('API', 'api')
  table.grantReadWriteData(api)
  bucket.grantReadWrite(api, 'inputs/*')
  bucket.grantRead(api, 'checkpoints/*')
  const url = api.addFunctionUrl({ authType: lambda.FunctionUrlAuthType.AWS_IAM })
  const control = fn('Engine', 'control', { CAPACITY_FUNCTION: capacity.functionName })
  capacity.grantInvoke(control)
  table.grantReadWriteData(control)
  bucket.grantReadWrite(control, 'checkpoints/*')
  bucket.grantReadWrite(control, 'artifacts/*')
  bucket.grantRead(control, 'results/*')
  nodeQueue.grantSendMessages(control)
  control.addEventSource(new sources.SqsEventSource(controlQueue, { batchSize: 1 }))
  const relay = fn('Relay', 'relay', { CAPACITY_FUNCTION: capacity.functionName })
  capacity.grantInvoke(relay)
  controlQueue.grantSendMessages(relay)
  table.grantReadWriteData(relay)
  relay.addEventSource(new sources.DynamoEventSource(table, { startingPosition: lambda.StartingPosition.TRIM_HORIZON,
    batchSize: 10, bisectBatchOnError: true, retryAttempts: 10,
    filters: [lambda.FilterCriteria.filter({ eventName: ['INSERT'], dynamodb: { Keys: { pk: { S: [{ prefix: 'RECEIPT#' }] } } } })] }))
  const deadletter = fn('NodeDeadletter', 'deadletter')
  bucket.grantReadWrite(deadletter, 'deadletters/*')
  bucket.grantReadWrite(deadletter, 'results/*')
  controlQueue.grantSendMessages(deadletter)
  deadletter.addEventSource(new sources.SqsEventSource(nodeDlq, { batchSize: 1 }))
  new CfnOutput(stack, 'PolicyProfile', { value: policyProfile })
  new CfnOutput(stack, 'PolicyFingerprint', { value: createHash('sha256').update(policyJson).digest('hex') })
  const outputs = { ApiUrl: url.url, ApiFunction: api.functionName, EngineFunction: control.functionName,
    RelayFunction: relay.functionName, CapacityFunction: capacity.functionName, AutoScalingGroup: pool.autoScalingGroupName,
    Bucket: bucket.bucketName, Table: table.tableName, ControlQueue: controlQueue.queueUrl, NodeQueue: nodeQueue.queueUrl,
    ControlDLQ: controlDlq.queueUrl, NodeDLQ: nodeDlq.queueUrl, DeadletterFunction: deadletter.functionName }
  for (const [key, value] of Object.entries(outputs)) {
    const output = new CfnOutput(stack, `${key}Output`, { value }); output.overrideLogicalId(key)
  }
  Aspects.of(app).add({ visit(construct) { if (construct instanceof CfnResource) construct.applyRemovalPolicy(RemovalPolicy.DESTROY) } })
}
