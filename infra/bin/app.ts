import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  App, Aspects, CfnOutput, CfnResource, Duration, RemovalPolicy, Stack, DockerBuildSecret,
  aws_ec2 as ec2, aws_ecr_assets as assets, aws_elasticloadbalancingv2 as elbv2,
  aws_elasticloadbalancingv2_targets as targets, aws_iam as iam,
  aws_logs as logs, aws_rds as rds, aws_s3 as s3, aws_sqs as sqs,
  aws_secretsmanager as secrets
} from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execFileSync('git', ['branch', '--show-current']).toString().trim()
if (!branch) throw new Error('Cannot deploy from detached HEAD')
const sanitized = branch.replace(/[^A-Za-z0-9-]/g, '-')
const prefix = /^[A-Za-z]/.test(sanitized) ? sanitized : `Branch-${sanitized}`
const stack = new Stack(app, `${prefix}-Judge`, { env: { region: 'ap-northeast-1' } })
const runtimeDir = resolve(dirname(fileURLToPath(import.meta.url)), '../../deploy/judge')
if (!existsSync(resolve(runtimeDir, '.build/traojudge-judge'))) throw new Error('Run infra/scripts/prepare.sh first')
const runtime = new assets.DockerImageAsset(stack, 'Runtime', {
  directory: runtimeDir, platform: assets.Platform.LINUX_AMD64,
  buildSecrets: process.env.CODEX_PROXY_CERT ? { proxy_ca: DockerBuildSecret.fromSrc(process.env.CODEX_PROXY_CERT) } : undefined
})
const vpc = new ec2.Vpc(stack, 'Network', {
  maxAzs: 2, natGateways: 0,
  subnetConfiguration: [
    { name: 'Public', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 },
    { name: 'Database', subnetType: ec2.SubnetType.PRIVATE_ISOLATED, cidrMask: 24 }
  ]
})
const controllerSecurity = new ec2.SecurityGroup(stack, 'ControllerSecurity', { vpc })
const workerSecurity = new ec2.SecurityGroup(stack, 'WorkerSecurity', { vpc })
const databaseSecurity = new ec2.SecurityGroup(stack, 'DatabaseSecurity', { vpc })
databaseSecurity.addIngressRule(controllerSecurity, ec2.Port.tcp(5432), 'Controller PostgreSQL only')
const credential = new secrets.Secret(stack, 'DatabaseCredential', {
  generateSecretString: { secretStringTemplate: JSON.stringify({ username: 'judge' }), generateStringKey: 'password', excludePunctuation: true }
})
const database = new rds.DatabaseInstance(stack, 'Database', {
  vpc, vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_ISOLATED }, securityGroups: [databaseSecurity],
  engine: rds.DatabaseInstanceEngine.postgres({ version: rds.PostgresEngineVersion.VER_16_9 }),
  instanceType: ec2.InstanceType.of(ec2.InstanceClass.T4G, ec2.InstanceSize.MICRO),
  credentials: rds.Credentials.fromSecret(credential), databaseName: 'judge',
  allocatedStorage: 20, storageEncrypted: true, backupRetention: Duration.days(0),
  deleteAutomatedBackups: true, deletionProtection: false, removalPolicy: RemovalPolicy.DESTROY,
  publiclyAccessible: false, multiAz: false
})
const bucket = new s3.Bucket(stack, 'Artifacts', {
  versioned: true, encryption: s3.BucketEncryption.S3_MANAGED,
  blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, enforceSSL: true,
  autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY,
  lifecycleRules: [{ abortIncompleteMultipartUploadAfter: Duration.days(1) }]
})
const requestDlq = new sqs.Queue(stack, 'RequestDlq', { encryption: sqs.QueueEncryption.SQS_MANAGED, retentionPeriod: Duration.days(1) })
const resultDlq = new sqs.Queue(stack, 'ResultDlq', { encryption: sqs.QueueEncryption.SQS_MANAGED, retentionPeriod: Duration.days(1) })
const requests = new sqs.Queue(stack, 'Requests', {
  encryption: sqs.QueueEncryption.SQS_MANAGED, enforceSSL: true,
  visibilityTimeout: Duration.seconds(900), receiveMessageWaitTime: Duration.seconds(20),
  deadLetterQueue: { queue: requestDlq, maxReceiveCount: 5 }, retentionPeriod: Duration.days(1)
})
const results = new sqs.Queue(stack, 'Results', {
  encryption: sqs.QueueEncryption.SQS_MANAGED, enforceSSL: true,
  visibilityTimeout: Duration.seconds(120), receiveMessageWaitTime: Duration.seconds(20),
  deadLetterQueue: { queue: resultDlq, maxReceiveCount: 5 }, retentionPeriod: Duration.days(1)
})
const logGroup = new logs.LogGroup(stack, 'JudgeLogs', { retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY })
function role(id: string) {
  const value = new iam.Role(stack, id, {
    assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
    managedPolicies: [iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMManagedInstanceCore')]
  })
  runtime.repository.grantPull(value)
  logGroup.grantWrite(value)
  return value
}
const controllerRole = role('ControllerRole')
const workerRole = role('WorkerRole')
bucket.grantRead(controllerRole)
bucket.grantRead(workerRole, 'inputs/*')
bucket.grantReadWrite(workerRole, 'runs/*')
requests.grantSendMessages(controllerRole)
results.grantConsumeMessages(controllerRole)
requests.grantConsumeMessages(workerRole)
results.grantSendMessages(workerRole)
credential.grantRead(controllerRole)
const common = {
  AWS_REGION: stack.region, AWS_DEFAULT_REGION: stack.region,
  ARTIFACT_BUCKET: bucket.bucketName, REQUEST_QUEUE_URL: requests.queueUrl, RESULT_QUEUE_URL: results.queueUrl
}
function host(id: string, hostRole: iam.Role, security: ec2.SecurityGroup, mode: 'controller' | 'worker') {
  const machine = new ec2.Instance(stack, id, {
    vpc, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
    instanceType: ec2.InstanceType.of(ec2.InstanceClass.T3, ec2.InstanceSize.MEDIUM),
    machineImage: ec2.MachineImage.latestAmazonLinux2023(),
    role: hostRole, securityGroup: security, requireImdsv2: true,
    blockDevices: [{ deviceName: '/dev/xvda', volume: ec2.BlockDeviceVolume.ebs(32, { encrypted: true, deleteOnTermination: true }) }]
  })
  const env: Record<string, string> = { ...common }
  if (mode === 'controller') {
    env.DATABASE_HOST = database.dbInstanceEndpointAddress
    env.DATABASE_SECRET_ARN = credential.secretArn
  }
  machine.userData.addCommands(
    'set -euo pipefail', 'dnf install -y docker',
    'command -v aws >/dev/null || dnf install -y awscli-2', 'systemctl enable --now docker',
    'install -d -m 0755 /etc/traojudge /var/lib/traojudge',
    `aws ecr get-login-password --region ${stack.region} | docker login --username AWS --password-stdin ${stack.account}.dkr.ecr.${stack.region}.amazonaws.com`,
    `for n in $(seq 1 30); do docker pull ${runtime.imageUri} && break; sleep 10; done`,
    `image=$(docker image inspect --format '{{index .RepoDigests 0}}' ${runtime.imageUri})`,
    'test -n "$image"',
    `cat > /etc/traojudge/${mode}.env <<'JUDGE_ENV'\n${Object.entries(env).map(([k, v]) => `${k}=${v}`).join('\n')}\nJUDGE_ENV`,
    `printf 'RUNTIME_IMAGE=%s\\n' "$image" >> /etc/traojudge/${mode}.env`,
    'printf "%s\\n" "$image" > /etc/traojudge/runtime-image',
    `docker run -d --restart always --name ${mode} --network host --env-file /etc/traojudge/${mode}.env --log-driver awslogs --log-opt awslogs-region=${stack.region} --log-opt awslogs-group=${logGroup.logGroupName} --log-opt awslogs-stream=${mode} ${mode === 'worker' ? '--mount type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock --mount type=bind,src=/var/lib/traojudge,dst=/var/lib/traojudge' : ''} "$image" ${mode === 'controller' ? 'traojudge-judge' : 'python3 /app/worker.py'}`
  )
  return machine
}
const controller = host('Controller', controllerRole, controllerSecurity, 'controller')
const worker = host('Worker', workerRole, workerSecurity, 'worker')
const loadBalancer = new elbv2.ApplicationLoadBalancer(stack, 'InternalApi', {
  vpc, internetFacing: false, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC }
})
const targetGroup = loadBalancer.addListener('Http', { port: 80 }).addTargets('Controller', {
  port: 8080, targets: [new targets.InstanceTarget(controller, 8080)],
  healthCheck: { path: '/health', interval: Duration.seconds(15) }
})
loadBalancer.connections.allowTo(controller, ec2.Port.tcp(8080), 'Internal ALB to controller API')
for (const [id, value] of Object.entries({
  ArtifactBucket: bucket.bucketName, RequestQueueUrl: requests.queueUrl,
  ResultQueueUrl: results.queueUrl, RequestDlqUrl: requestDlq.queueUrl, ResultDlqUrl: resultDlq.queueUrl,
  ControllerInstanceId: controller.instanceId, WorkerInstanceId: worker.instanceId,
  ControllerTargetGroupArn: targetGroup.targetGroupArn,
  InternalApiUrl: `http://${loadBalancer.loadBalancerDnsName}`, LogGroupName: logGroup.logGroupName
})) new CfnOutput(stack, id, { value })
Aspects.of(app).add({ visit(construct) {
  if (construct instanceof CfnResource && construct.cfnResourceType === 'AWS::Lambda::Function') {
    construct.addPropertyOverride('LoggingConfig.LogGroup', logGroup.logGroupName)
  }
  if (construct instanceof CfnResource) construct.applyRemovalPolicy(RemovalPolicy.DESTROY)
} })
