import { App } from 'aws-cdk-lib'
import { buildPool } from './pool-stack.js'

buildPool(new App({ analyticsReporting: false }))
