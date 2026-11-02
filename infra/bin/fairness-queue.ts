import { execSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { App, Aspects, CfnOutput, CfnResource, Duration, RemovalPolicy, Stack,
  aws_iam as iam, aws_lambda as lambda, aws_lambda_event_sources as sources,
  aws_logs as logs, aws_s3 as s3, aws_sqs as sqs } from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Use a codex- branch')
const stack = new Stack(app, `${branch}-FairnessQueue`, { env: { region: 'ap-northeast-1' } })
// Existing VMs remain owned by their CloudFormation stack; this worker cannot create VMs.
const outputs = Object.values(JSON.parse(readFileSync('../work/fairness-outputs.json', 'utf8')))[0] as Record<string, string>
const dlq = new sqs.Queue(stack, 'Deadletters', { fifo: true, retentionPeriod: Duration.days(14) })
const queue = new sqs.Queue(stack, 'Submissions', { fifo: true, visibilityTimeout: Duration.seconds(30),
  deadLetterQueue: { queue: dlq, maxReceiveCount: 3 } })
const bucket = new s3.Bucket(stack, 'Evidence', { versioned: true, enforceSSL: true,
  blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY })
const logGroup = new logs.LogGroup(stack, 'Logs', { retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY })
const fn = new lambda.Function(stack, 'Worker', { runtime: lambda.Runtime.PYTHON_3_13,
  handler: 'queued.handler', code: lambda.Code.fromAsset('../work/fairness-provider'),
  memorySize: 512, timeout: Duration.seconds(25), logGroup,
  environment: { BUCKET: bucket.bucketName, VM: outputs['VM0Id'] } })
bucket.grantReadWrite(fn)
fn.addToRolePolicy(new iam.PolicyStatement({ actions: ['lambda:ResumeMicrovm', 'lambda:SuspendMicrovm',
  'lambda:GetMicrovm', 'lambda:CreateMicrovmAuthToken'], resources: ['*'] }))
fn.addEventSource(new sources.SqsEventSource(queue, { batchSize: 1 }))
for (const [key, value] of Object.entries({ Queue: queue.queueUrl, DLQ: dlq.queueUrl, Bucket: bucket.bucketName, Fifo: 'true' }))
  new CfnOutput(stack, key, { value })
Aspects.of(app).add({ visit(node) { if (node instanceof CfnResource) node.applyRemovalPolicy(RemovalPolicy.DESTROY) } })
