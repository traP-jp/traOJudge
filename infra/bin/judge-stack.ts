import { execSync } from 'node:child_process'
import path from 'node:path'
import {
  App, Aspects, CfnOutput, CfnResource, DockerBuildSecret, Duration, RemovalPolicy, Stack,
  aws_batch as batch, aws_dynamodb as dynamodb, aws_ec2 as ec2,
  aws_ecr_assets as assets, aws_events as events, aws_events_targets as targets,
  aws_iam as iam, aws_lambda as lambda, aws_lambda_event_sources as sources,
  aws_logs as logs, aws_s3 as s3, aws_s3_notifications as notifications, aws_sqs as sqs,
} from 'aws-cdk-lib'

export function buildFargate(app: App) {
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Use a codex-<topic> branch for sandbox deployment.')
const prefix = branch.replace(/[^A-Za-z0-9-]/g, '-')
const proofName: string = app.node.tryGetContext('proofName') ?? 'Judge'
if (!/^[A-Za-z][A-Za-z0-9-]*$/.test(proofName)) throw new Error('Invalid proofName.')
const stack = new Stack(app, `${prefix}-${proofName}`, {
  env: { region: 'ap-northeast-1' },
  description: 'Scale-to-zero native judge infrastructure proof; no product UI.',
})
const root = process.cwd()
const certificate = process.env.CODEX_PROXY_CERT
const secrets: Record<string, string> = certificate ? { proxy_ca: DockerBuildSecret.fromSrc(certificate) } : {}
const workerImage = new assets.DockerImageAsset(stack, 'WorkerImage', {
  directory: path.join(root, 'worker'), buildSecrets: secrets,
  platform: assets.Platform.LINUX_AMD64,
})
const runtimeImage = lambda.DockerImageCode.fromImageAsset(path.join(root, 'runtime'), {
  buildSecrets: secrets, platform: assets.Platform.LINUX_AMD64,
})
const bucket = new s3.Bucket(stack, 'Files', {
  versioned: true, enforceSSL: true, blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
  removalPolicy: RemovalPolicy.DESTROY, autoDeleteObjects: true,
})
const table = new dynamodb.Table(stack, 'Acceptance', {
  partitionKey: { name: 'pk', type: dynamodb.AttributeType.STRING },
  billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
  stream: dynamodb.StreamViewType.NEW_IMAGE,
  removalPolicy: RemovalPolicy.DESTROY,
})
const controlDlq = new sqs.Queue(stack, 'ControlDLQ', { fifo: true, retentionPeriod: Duration.days(14) })
const nodeDlq = new sqs.Queue(stack, 'NodeDLQ', { retentionPeriod: Duration.days(14) })
const resultDlq = new sqs.Queue(stack, 'ResultDLQ', { retentionPeriod: Duration.days(14) })
const controlQueue = new sqs.Queue(stack, 'ControlQueue', {
  fifo: true, visibilityTimeout: Duration.minutes(3),
  deadLetterQueue: { queue: controlDlq, maxReceiveCount: 5 },
})
const nodeQueue = new sqs.Queue(stack, 'NodeQueue', {
  visibilityTimeout: Duration.minutes(3), deadLetterQueue: { queue: nodeDlq, maxReceiveCount: 5 },
})
const resultQueue = new sqs.Queue(stack, 'ResultQueue', {
  visibilityTimeout: Duration.minutes(3), deadLetterQueue: { queue: resultDlq, maxReceiveCount: 5 },
})
bucket.addEventNotification(s3.EventType.OBJECT_CREATED, new notifications.SqsDestination(resultQueue),
  { prefix: 'results/', suffix: '.json' })

const vpc = new ec2.Vpc(stack, 'ExecutionVPC', {
  maxAzs: 2, natGateways: 0,
  subnetConfiguration: [{ name: 'Execution', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 }],
})
const securityGroup = new ec2.SecurityGroup(stack, 'WorkerNetwork', {
  vpc, allowAllOutbound: false, description: 'Supervisor HTTPS only; workload sockets denied by seccomp.',
})
securityGroup.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443), 'Image pull, logs, scoped S3 URLs')
const executionRole = new iam.Role(stack, 'ImagePullRole', {
  assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com'),
  managedPolicies: [iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AmazonECSTaskExecutionRolePolicy')],
})
workerImage.repository.grantPull(executionRole)
const workerLog = new logs.LogGroup(stack, 'WorkerLogs', { retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY })
const compute = new batch.CfnComputeEnvironment(stack, 'Compute', {
  type: 'MANAGED', state: 'ENABLED',
  computeResources: { type: 'FARGATE', maxvCpus: 8,
    subnets: vpc.publicSubnets.map(subnet => subnet.subnetId), securityGroupIds: [securityGroup.securityGroupId] },
})
const jobQueue = new batch.CfnJobQueue(stack, 'JobQueue', {
  priority: 1, state: 'ENABLED', computeEnvironmentOrder: [{ order: 1, computeEnvironment: compute.ref }],
})
const job = new batch.CfnJobDefinition(stack, 'NativeNode', {
  type: 'container', platformCapabilities: ['FARGATE'],
  timeout: { attemptDurationSeconds: 120 }, retryStrategy: { attempts: 1 },
  containerProperties: {
    image: workerImage.imageUri, executionRoleArn: executionRole.roleArn,
    // Deliberately no jobRoleArn: workloads cannot obtain AWS credentials.
    resourceRequirements: [{ type: 'VCPU', value: '0.5' }, { type: 'MEMORY', value: '1024' }],
    networkConfiguration: { assignPublicIp: 'ENABLED' },
    fargatePlatformConfiguration: { platformVersion: '1.4.0' },
    readonlyRootFilesystem: true, user: '0',
    volumes: [{ name: 'work' }], mountPoints: [{ sourceVolume: 'work', containerPath: '/work', readOnly: false }],
    logConfiguration: { logDriver: 'awslogs', options: {
      'awslogs-group': workerLog.logGroupName, 'awslogs-region': stack.region, 'awslogs-stream-prefix': 'node',
    } },
  },
})
const environment = { BUCKET: bucket.bucketName, TABLE: table.tableName,
  CONTROL_QUEUE: controlQueue.queueUrl, NODE_QUEUE: nodeQueue.queueUrl, STACK_PREFIX: prefix }
function runtime(id: string, mode: string, extra: Record<string, string> = {}) {
  const logGroup = new logs.LogGroup(stack, `${id}Logs`, { retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY })
  return new lambda.DockerImageFunction(stack, id, {
    code: runtimeImage, timeout: Duration.seconds(30), memorySize: 512, logGroup,
    environment: { ...environment, MODE: mode, ...extra },
  })
}
const api = runtime('API', 'api')
table.grantReadWriteData(api)
bucket.grantReadWrite(api, 'inputs/*')
bucket.grantRead(api, 'checkpoints/*')
const apiUrl = api.addFunctionUrl({ authType: lambda.FunctionUrlAuthType.AWS_IAM })
const control = runtime('Engine', 'control')
bucket.grantReadWrite(control, 'checkpoints/*')
bucket.grantRead(control, 'results/*')
nodeQueue.grantSendMessages(control)
control.addEventSource(new sources.SqsEventSource(controlQueue, { batchSize: 1 }))
const relay = runtime('Relay', 'relay')
table.grantReadWriteData(relay)
bucket.grantReadWrite(relay, 'results/*')
controlQueue.grantSendMessages(relay)
relay.addEventSource(new sources.DynamoEventSource(table, {
  startingPosition: lambda.StartingPosition.TRIM_HORIZON, batchSize: 10,
  bisectBatchOnError: true, retryAttempts: 10,
  filters: [lambda.FilterCriteria.filter({ eventName: ['INSERT'], dynamodb: { Keys: { pk: { S: [{ prefix: 'RECEIPT#' }] } } } })],
}))
relay.addEventSource(new sources.SqsEventSource(resultQueue, { batchSize: 1 }))
const dispatcher = runtime('Dispatcher', 'dispatch', { JOB_QUEUE: jobQueue.ref, JOB_DEFINITION: job.ref })
table.grantReadWriteData(dispatcher)
bucket.grantReadWrite(dispatcher)
dispatcher.addToRolePolicy(new iam.PolicyStatement({ actions: ['batch:SubmitJob'], resources: [jobQueue.ref, job.ref] }))
dispatcher.addEventSource(new sources.SqsEventSource(nodeQueue, { batchSize: 1, maxConcurrency: 4 }))
new events.Rule(stack, 'FailedTasks', {
  eventPattern: { source: ['aws.batch'], detailType: ['Batch Job State Change'], detail: { jobQueue: [jobQueue.ref], status: ['FAILED'] } },
  targets: [new targets.LambdaFunction(relay)],
})

for (const [name, value] of Object.entries({ ApiUrl: apiUrl.url, ApiFunction: api.functionName,
  EngineFunction: control.functionName, RelayFunction: relay.functionName, DispatcherFunction: dispatcher.functionName,
  Bucket: bucket.bucketName, Table: table.tableName, JobQueue: jobQueue.ref, JobDefinition: job.ref,
  ControlQueue: controlQueue.queueUrl, NodeQueue: nodeQueue.queueUrl, ResultQueue: resultQueue.queueUrl,
  ControlDLQ: controlDlq.queueUrl, NodeDLQ: nodeDlq.queueUrl, ResultDLQ: resultDlq.queueUrl })) {
  const output = new CfnOutput(stack, `${name}Output`, { value })
  output.overrideLogicalId(name)
}
Aspects.of(app).add({ visit(construct) {
  if (construct instanceof CfnResource) construct.applyRemovalPolicy(RemovalPolicy.DESTROY)
} })

}
