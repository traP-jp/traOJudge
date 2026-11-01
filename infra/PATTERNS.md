# 動作確認用の AWS アカウント

このリポジトリで作業するエージェントは、動作確認のために用意された AWS アカウントに、CDK でスタックを作成できる。
この AWS アカウントの認証情報は、作業環境に設定済みである。
リージョンは ap-northeast-1 である。
この AWS アカウントは、並行して動く他のタスクと共有している。

## スタックの作成から削除まで

エージェントは、動作を確かめる必要が生じたときにスタックを作成する。
動作確認を終えたら、成否にかかわらず、そのスタックを削除する。
スタックは、作成から 1 時間たつと削除される。

```sh
cd infra
npm ci
npx aws-cdk deploy --all --require-approval never
# ここで動作を確かめる
npx aws-cdk destroy --all --force
```

- 作業中にスタックが削除されていたら、deploy し直す。
- deploy する前から同じ名前のスタックがあれば、destroy してから deploy する。
- destroy に失敗したら、スタック名とエラーを報告する。

リソースは CDK で作成する。
CDK の外で作成したリソースは、スタックを削除しても消えない。
AWS CLI は、スタックが作成したリソースの確認と、動作確認のためのデータの読み書きに使う。
権限エラーが出たら、回避を試みずに報告する。

## 規則

### スタック名の先頭に、作業中のブランチ名を付ける

この AWS アカウントは複数のタスクが共有している。
他のタスクが同じ名前のスタックを deploy すると、CloudFormation が既存のスタックを更新し、互いのリソースを書き換えてしまう。

```ts
// infra/bin/app.ts
// Do
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
```

### リソースの物理名を指定しない

S3 バケット、SQS キュー、IAM ロールなどに固定の物理名を指定すると、他のタスクや他の AWS アカウントのリソースと名前が衝突しうる。
物理名を省略すると、CloudFormation がスタック名を含む一意な名前をリソースごとに生成する。

```ts
// infra/bin/app.ts
// Do
const queue = new sqs.Queue(stack, 'ExampleQueue', {
  removalPolicy: RemovalPolicy.DESTROY
})
new CfnOutput(stack, 'QueueUrl', { value: queue.queueUrl })
```

`ExampleQueue` は CDK のコンストラクトを識別する名前であり、SQS キューの物理名ではない。

### すべてのリソースに DESTROY を適用する

CloudFormation は、削除ポリシーが `RETAIN` のリソースをスタックの削除後も残す。

```ts
// infra/bin/app.ts
// Do
const queue = new sqs.Queue(stack, 'ExampleQueue', {
  removalPolicy: RemovalPolicy.DESTROY
})
new CfnOutput(stack, 'QueueUrl', { value: queue.queueUrl })
```

```ts
// infra/bin/app.ts
// Do
Aspects.of(app).add({
  visit(construct) {
    if (construct instanceof CfnResource) {
      construct.applyRemovalPolicy(RemovalPolicy.DESTROY)
    }
  }
})
```

S3 バケットを追加する場合は、エージェントは `autoDeleteObjects: true` も指定し、オブジェクトと過去の版をスタックと一緒に削除する。

### 作業中に知った AWS アカウントの情報を、公開される場所に書かない

このリポジトリは公開されている。
作業中に AWS アカウントの ID や、リソースの ARN、URL を知った場合も、それらをファイル、commit メッセージ、PR に書かない。
