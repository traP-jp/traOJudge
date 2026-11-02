import { execSync } from 'node:child_process'
import { App, Aspects, CfnOutput, CfnResource, Duration, RemovalPolicy, Stack,
  aws_lambda as lambda, aws_logs as logs } from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execSync('git branch --show-current').toString().trim()
if (!branch.startsWith('codex-')) throw new Error('Check out a codex-<topic> branch first.')
const stack = new Stack(app, `${branch.replace(/[^A-Za-z0-9-]/g, '-')}-LambdaSmoke`, {
  env: { region: 'ap-northeast-1' },
})
const logGroup = new logs.LogGroup(stack, 'Logs', {
  retention: logs.RetentionDays.ONE_DAY, removalPolicy: RemovalPolicy.DESTROY,
})
const fn = new lambda.Function(stack, 'Echo', {
  runtime: lambda.Runtime.PYTHON_3_13, handler: 'index.handler',
  memorySize: 128, timeout: Duration.seconds(5), logGroup,
  code: lambda.Code.fromInline([
    'def handler(event, context):',
    '    return {"ok": True, "received": event}',
  ].join('\n')),
})
new CfnOutput(stack, 'FunctionName', { value: fn.functionName })
Aspects.of(app).add({
  visit(node) { if (node instanceof CfnResource) node.applyRemovalPolicy(RemovalPolicy.DESTROY) },
})
