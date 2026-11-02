import { execSync } from 'node:child_process'
import { App, Aspects, CfnOutput, CfnResource, CustomResource, Duration, Fn, RemovalPolicy, Stack,
  aws_ec2 as ec2, aws_iam as iam, aws_lambda as lambda, aws_logs as logs, aws_s3 as s3, aws_s3_assets as assets,
  custom_resources as custom } from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Use a codex- branch')
const stack = new Stack(app, `${branch.replace(/[^A-Za-z0-9-]/g, '-')}-Fairness`, { env: { region: 'ap-northeast-1' } })
const bucket = new s3.Bucket(stack, 'Evidence', { blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
  enforceSSL: true, versioned: true, autoDeleteObjects: true, removalPolicy: RemovalPolicy.DESTROY })
const source = new assets.Asset(stack, 'Source', { path: 'experiments/fairness',
  exclude: ['__pycache__', 'provider.py', 'prepare.py', 'run.py', 'agent.py'] })
const buildRole = new iam.Role(stack, 'BuildRole', { assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com') })
buildRole.assumeRolePolicy!.addStatements(new iam.PolicyStatement({
  principals: [new iam.ServicePrincipal('lambda.amazonaws.com')], actions: ['sts:TagSession'] }))
source.grantRead(buildRole)
const imageLogs = new logs.LogGroup(stack, 'ImageLogs', { retention: logs.RetentionDays.ONE_DAY,
  removalPolicy: RemovalPolicy.DESTROY })
imageLogs.grantWrite(buildRole)
// The service requires Name. Its suffix comes from CloudFormation's generated stack ID.
const suffix = Fn.select(0, Fn.split('-', Fn.select(2, Fn.split('/', stack.stackId))))
const image = new lambda.CfnMicrovmImage(stack, 'Image', {
  name: Fn.join('-', [stack.stackName, suffix]),
  baseImageArn: 'arn:aws:lambda:ap-northeast-1:aws:microvm-image:al2023-1', baseImageVersion: '2',
  buildRoleArn: buildRole.roleArn, description: 'Allowlisted C fairness fixtures; no arbitrary submission API',
  codeArtifact: { uri: `s3://${source.s3BucketName}/${source.s3ObjectKey}` },
  logging: { cloudWatch: { logGroup: imageLogs.logGroupName } },
  egressNetworkConnectors: ['arn:aws:lambda:ap-northeast-1:aws:network-connector:aws-network-connector:INTERNET_EGRESS'],
  cpuConfigurations: [{ architecture: 'ARM_64' }], resources: [{ minimumMemoryInMiB: 2048 }],
  additionalOsCapabilities: [], environmentVariables: [],
  hooks: { port: 8080, microvmImageHooks: { ready: 'ENABLED', validate: 'ENABLED',
    readyTimeoutInSeconds: 120, validateTimeoutInSeconds: 120 } },
})
image.node.addDependency(buildRole)
const handlerLogs = new logs.LogGroup(stack, 'HandlerLogs', { retention: logs.RetentionDays.ONE_DAY,
  removalPolicy: RemovalPolicy.DESTROY })
const handler = new lambda.Function(stack, 'Owner', { runtime: lambda.Runtime.PYTHON_3_13,
  handler: 'provider.handler', code: lambda.Code.fromAsset('../work/fairness-provider'),
  memorySize: 512, timeout: Duration.minutes(3), logGroup: handlerLogs })
bucket.grantWrite(handler)
handler.addToRolePolicy(new iam.PolicyStatement({ actions: ['lambda:RunMicrovm', 'lambda:GetMicrovm',
  'lambda:TerminateMicrovm', 'lambda:CreateMicrovmAuthToken', 'lambda:PassNetworkConnector'], resources: ['*'] }))
const providerLogs = new logs.LogGroup(stack, 'ProviderLogs', { retention: logs.RetentionDays.ONE_DAY,
  removalPolicy: RemovalPolicy.DESTROY })
const provider = new custom.Provider(stack, 'Provider', { onEventHandler: handler, logGroup: providerLogs })
const isolated = new ec2.Vpc(stack, 'NoEgressVPC', { maxAzs: 1, natGateways: 0,
  subnetConfiguration: [{ name: 'Isolated', subnetType: ec2.SubnetType.PRIVATE_ISOLATED, cidrMask: 24 }] })
const blocked = new ec2.SecurityGroup(stack, 'NoEgressNetwork', { vpc: isolated, allowAllOutbound: false })
const networkRole = new iam.Role(stack, 'NetworkRole', { assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
  managedPolicies: [iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaVPCAccessExecutionRole')] })
const connector = new lambda.CfnNetworkConnector(stack, 'NoEgressConnector', {
  operatorRole: networkRole.roleArn, configuration: { vpcEgressConfiguration: {
    subnetIds: isolated.isolatedSubnets.map(s => s.subnetId), securityGroupIds: [blocked.securityGroupId],
    associatedComputeResourceTypes: ['MicroVm'], networkProtocol: 'IPv4' } } })
for (let i = 0; i < 3; i++) {
  const vm = new CustomResource(stack, `VM${i}`, { serviceToken: provider.serviceToken,
    properties: { Image: image.attrImageArn, Version: image.attrLatestActiveImageVersion,
      Label: `microvm-${i}`, Bucket: bucket.bucketName, Egress: connector.attrLatestVersionArn } })
  new CfnOutput(stack, `VM${i}Id`, { value: vm.ref })
  new CfnOutput(stack, `VM${i}Endpoint`, { value: vm.getAttString('Endpoint') })
}
new CfnOutput(stack, 'Bucket', { value: bucket.bucketName })
new CfnOutput(stack, 'ImageArn', { value: image.attrImageArn })
new CfnOutput(stack, 'BuildLogGroup', { value: imageLogs.logGroupName })
Aspects.of(app).add({ visit(node) { if (node instanceof CfnResource) node.applyRemovalPolicy(RemovalPolicy.DESTROY) } })
