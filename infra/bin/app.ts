import { execSync } from 'node:child_process'
import {
  App,
  Aspects,
  CfnOutput,
  CfnResource,
  RemovalPolicy,
  Stack,
  aws_sqs as sqs
} from 'aws-cdk-lib'

const app = new App({ analyticsReporting: false })
const branch = execSync('git branch --show-current').toString().trim()
if (!branch) {
  throw new Error('Cannot create stacks from a detached HEAD. Check out a branch first.')
}
const sanitizedBranch = branch.replace(/[^A-Za-z0-9-]/g, '-')
const stackPrefix = /^[A-Za-z]/.test(sanitizedBranch)
  ? sanitizedBranch
  : `Branch-${sanitizedBranch}`

const stack = new Stack(app, `${stackPrefix}-Example`, {
  env: { region: 'ap-northeast-1' }
})
const queue = new sqs.Queue(stack, 'ExampleQueue', {
  removalPolicy: RemovalPolicy.DESTROY
})
new CfnOutput(stack, 'QueueUrl', { value: queue.queueUrl })

Aspects.of(app).add({
  visit(construct) {
    if (construct instanceof CfnResource) {
      construct.applyRemovalPolicy(RemovalPolicy.DESTROY)
    }
  }
})
