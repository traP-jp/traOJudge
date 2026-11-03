# 独立提案B：作問者ASL＋Step Functions Standard＋AWS Batch

2026-11-03 JST。GPT-6 Astra / Maxによる独立した構成提案。
既存リポジトリ、過去の会話、他の提案を参照せず、公開一次資料だけで策定。
本レポートはその提案を整理したもの。コード編集・deploy・実測は行っていない。

## 受領した条件

AWS内で完結するproduction構成。提出順が守られるが、実行順は自由。
durable execution、isolated execution、real-time進捗表示、EC2最大5台を満たす。
作問者がフローをデータとして登録でき、問題ごとのリポジトリ・IaC変更は不要。
Step Functionsのフロー自体のcustomも候補に含める。
consumer group相当、最終判定1回またはDLQ、再起動・master死亡・worker死亡を説明する。
既成AWSサービスの責務を使い、独自機構を増やしすぎない。

## 提案

**Step Functions Standard＋AWS Batch＋EC2＋DynamoDB＋S3＋AppSync Events。**

| 責務 | 担当 |
| --- | --- |
| 受理順・冪等キー・起動待ち | DynamoDBの受付トランザクション |
| 入力・問題bundle・attempt別成果物 | S3 |
| 分岐・待機・依存する処理の起動 | Step Functions Standard |
| job保持・配置・worker障害時の再試行 | AWS Batch queue / scheduler / RetryStrategy |
| 敵対的プログラムの実行 | EC2上のtrusted runner＋Firecracker等のmicroVM |
| 最終判定1件または終端失敗 | DynamoDBの条件付きterminal確定 |
| 進捗の正本・通知 | DynamoDB snapshot＋Streams＋AppSync Events |
| 起動漏れ・期限超過等の回収 | 定期Reconciler |

**この独立提案はSQSを使用しない。** DLQ相当はDynamoDBの`DEAD_LETTER`という永続失敗台帳。
理由、入力参照、workflow版、execution参照を保持し、再判定の起点にする。
SQSへの配送を含む構成を、この提案そのものとは扱わない。

## 作問者のフロー

1. 作問者がASLと、テスト・checker・実行script等のbundleを登録する。
2. 登録APIが構文、呼出先、時間、並列数、入出力の制約を検証する。
3. AWS APIで問題別のStandard state machineを作成・更新し、不変versionを公開する。
4. 問題版にversion ARNとbundle hashを保存し、提出はその版に固定する。

作問者は`Choice / Parallel / Map / Wait / Retry / Catch`と接続関係を定義する。
Taskの呼出先は共通SandboxTaskだけに制限する。
共通Taskは固定queue・job definitionへ`batch:submitJob.sync`し、成功attemptのS3 manifestを検証する。
Batchの完了response自体をjudge結果とは扱わない。
ホストcommand、IAM role、queue、job definitionは作問者に選ばせず、プログラムはmicroVM内で実行する。
構文検証と権限・資源上限の検証は別々に必要。[S1][S2][S3]

## 受付・実行・確定

scope内counterの条件付き更新、提出行、冪等キー、`START_PENDING`をDynamoDB transactionで保存する。
`submissionSeq`はそのcommit順。画面もその順に表示する。[S4]

Streamsの短い起動Lambdaが、判定ID・generationから固定したnameと同一inputでSupervisorを開始する。
Supervisorは問題別workflowを`.sync:2`で待ち、問題別workflowは共通SandboxTaskを呼ぶ。
Standardの`StartExecution`は実行中の同一name・inputに対して冪等。
終了後は`ExecutionAlreadyExists`、90日後はname再利用が可能なので、永続台帳も併用する。[S5]

Batchはjobを保持・配置し、EC2死亡を含む失敗を有限回再試行する。
アプリ側にworker lease、leader election、masterプロセスを置かない。[S6][S7]
プログラムの再実行は許容し、`(submissionId, generation)`のterminal recordだけを条件付きで一度確定する。
WA / CE / TLE等は正常なjudge結果とし、基盤障害と分ける。
workerには提出terminalの更新権限を与えず、古いattempt・generationの成果物を混ぜない。

## 停止と回復

| 状況 | 回復手段 |
| --- | --- |
| 受理commit後にAPI死亡 | 台帳に残る起動待ちをStreamsまたはReconcilerが回収 |
| StartExecution成功後に応答消失 | 固定name・inputと既存executionを照合 |
| アプリ再起動・master死亡 | 進行状態はStep Functions / Batchに残る |
| worker / EC2死亡 | Batchがretry budget内で別attemptを配置 |
| 成果物保存後にworker死亡 | job ID・attempt別S3 pathから成功attemptだけを採用 |
| queue・image pullが停滞 | 受付時に固定した全体期限で失敗終端 |
| 成功と期限切れが競合 | 同じterminal keyの条件付き作成で一方だけ確定 |
| workflow自体が失敗・timeout・停止 | 独立Reconcilerが同じ確定処理で回収 |
| 通知欠落 | 台帳照合とUI snapshotの初回・再接続・定期取得 |

`RUNNABLE`は無期限に滞留し得て、Batch timeoutは`STARTING`を含まない。[S6][S8]
Streams保持は24時間、SF→EventBridge通知はbest effortなので、通知と独立した照合が必要。[S9][S10]
`.sync`取消しもbest effort。terminal確定と残存jobの回収を分ける。[S11]
eventual終端はAWSの復旧、定期照合の再開、必要な権限とデータ保持を前提とする。

## 隔離・台数・進捗

提出・compiler・checkerをmicroVM内へ置き、AWS資格情報、IMDS、host filesystem、外部networkを渡さない。
trusted runnerが入出力と資源制限を担う。EC2上のECS containerだけを敵対的コードの隔離境界とはしない。[S12]
C7i等のnested virtualizationは選択肢だが、KVM・Firecrackerの統合と運用は実装責任が残る。[S13][S14]
Batch timeoutは最低60秒であり、細かな判定時間制限はrunner側で実装する。[S8]

5台制御の候補は単一instance type＋On-Demand `BEST_FIT`＋`maxvCpus=5q`（qは1台分のvCPU数）。
`BEST_FIT_PROGRESSIVE`等は最大1台分超過し得る。
`BEST_FIT`のinfrastructure update制約とcompute environment交換時の新旧合計も管理する。[S15]

進捗には判定ID・generation・task ID・attempt・eventSeqを付け、保存してからAppSync Eventsへ配信する。
UIはsnapshotで復元し、重複・古いattemptを除外する。[S16]

## 未検証

対象region / instance typeでのBatch＋nested KVM＋Firecracker、sandboxの攻撃耐性、障害回復、
台数上限、起動時間、公平性、進捗遅延、処理能力、費用、quota適合は実機未検証。
公開仕様と設計手順を確認した提案であり、要求適合を証明したものではない。

## 一次資料

- [S1: ASL検証API](https://docs.aws.amazon.com/step-functions/latest/apireference/API_ValidateStateMachineDefinition.html)
- [S2: 不変state machine version](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-state-machine-version.html)
- [S3: Batch連携](https://docs.aws.amazon.com/step-functions/latest/dg/connect-batch.html)
- [S4: DynamoDB transactions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html)
- [S5: StartExecution](https://docs.aws.amazon.com/step-functions/latest/apireference/API_StartExecution.html)
- [S6: Batch job states](https://docs.aws.amazon.com/batch/latest/userguide/job_states.html)
- [S7: Batch retries](https://docs.aws.amazon.com/batch/latest/userguide/job_retries.html)
- [S8: Batch timeout](https://docs.aws.amazon.com/batch/latest/userguide/job_timeouts.html)
- [S9: DynamoDB Streams保持](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/HowItWorks.CoreComponents.html)
- [S10: Step Functions EventBridge通知](https://docs.aws.amazon.com/step-functions/latest/dg/eventbridge-integration.html)
- [S11: 同期統合と取消し](https://docs.aws.amazon.com/step-functions/latest/dg/connect-to-resource.html)
- [S12: ECS task IAMと隔離](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task-iam-roles.html)
- [S13: EC2 nested virtualization](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/amazon-ec2-nested-virtualization.html)
- [S14: Firecracker host設定](https://github.com/firecracker-microvm/firecracker/blob/main/docs/prod-host-setup.md)
- [S15: Batch compute resource制約](https://docs.aws.amazon.com/batch/latest/APIReference/API_ComputeResource.html)
- [S16: AppSync Events](https://docs.aws.amazon.com/appsync/latest/eventapi/event-api-concepts.html)
