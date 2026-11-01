import { App } from 'aws-cdk-lib'
import { buildFargate } from './judge-stack.js'
import { buildPool } from './pool-stack.js'
const app = new App({ analyticsReporting: false })
if (app.node.tryGetContext('execution') === 'fargate') buildFargate(app)
else buildPool(app)
