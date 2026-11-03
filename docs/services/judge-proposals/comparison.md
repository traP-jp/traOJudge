# 独立提案の比較

2026-11-03 JST。
[A：固定workflow＋DSL＋SQS](sqs-callback.md) / [B：作問者ASL＋Batch](native-asl-batch.md)。

「提案」はそのレポートに書かれた設計、「仕様」はリンクした一次資料で確認するサービスの契約、
「実測」は保存済み実験データを指す。提案の記述だけで要求適合とは判定しない。

## 設計上の差分

以下は両レポートの記載事項の対照であり、実装量・信頼性・性能の採点ではない。

| 項目 | Aの提案 | Bの提案 |
| --- | --- | --- |
| 作問者フロー | JSON等の独自DSL。固定workflow＋interpreterで実行 | ASLを登録APIで問題別Standard workflowとして公開 |
| 依存する処理の選択 | PlanNextとnode状態管理を実装 | ASLのChoice / Parallel / Map等を利用 |
| workerへの配送 | SQS Standard、workerがlong polling | Batch job queue / scheduler |
| worker失敗時の再試行 | SQS visibilityと独自lease / fencing | Batch RetryStrategy |
| workflowの待機 | SQS callback | 共通SandboxTaskによるBatch `.sync` |
| 終端失敗 | DynamoDB失敗台帳＋SQS judgement DLQ | DynamoDB `DEAD_LETTER`台帳。SQSなし |
| node配送失敗 | transport DLQとbridge | Batch job FAILEDをworkflowへ返す |
| 台数制御 | EC2 ASG。開始2台は提案者の選択 | Batch compute environment。単一型＋BEST_FITを候補化 |
| 最終判定 | DynamoDB条件付き確定 | DynamoDB条件付き確定 |
| 隔離 | EC2上のFirecracker等 | EC2上のFirecracker等 |
| 進捗の正本・通知 | DynamoDB snapshot＋AppSync Events | DynamoDB snapshot＋AppSync Events |
| 脱落の回収 | outbox / execution照合 | 起動待ち / execution / deadline照合 |

## サービス仕様の根拠

| 確認できる仕様 | 設計への制約 | 一次資料 |
| --- | --- | --- |
| SQS Standardはat-least-once配送 | Aでは重複を扱う必要がある。visibilityだけで物理実行1回とはならない | [SQS](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/standard-queues-at-least-once-delivery.html) |
| SFはSQS callbackとBatch `.sync`を提供 | 両案ともmanagedな待機を使える。外部処理の物理実行1回の証明にはならない | [callback](https://docs.aws.amazon.com/step-functions/latest/dg/connect-sqs.html) / [Batch](https://docs.aws.amazon.com/step-functions/latest/dg/connect-batch.html) |
| ASLをAPIのdefinitionに渡せ、公開versionは不変 | Bは問題ごとのリポジトリ・IaC変更を使わず登録できる。安全な呼出先の検証は別途必要 | [CreateStateMachine](https://docs.aws.amazon.com/step-functions/latest/apireference/API_CreateStateMachine.html) / [version](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-state-machine-version.html) |
| BatchはEC2障害等のretryを扱う | Bのworker再試行はBatchへ委譲可能。terminal確定は別処理 | [retry](https://docs.aws.amazon.com/batch/latest/userguide/job_retries.html) |
| Batch RUNNABLEは滞留し得て、timeoutはSTARTINGを含まない | Bにも受付からの絶対期限・回収が必要 | [状態](https://docs.aws.amazon.com/batch/latest/userguide/job_states.html) / [timeout](https://docs.aws.amazon.com/batch/latest/userguide/job_timeouts.html) |
| Batchの一部allocation strategyはmaxvCpusを最大1台分超過し得る。ASGもrebalancing中の超過がある | 両案とも単純な容量値だけで厳密な5台上限を証明しない | [Batch](https://docs.aws.amazon.com/batch/latest/APIReference/API_ComputeResource.html) / [ASG](https://docs.aws.amazon.com/autoscaling/ec2/userguide/auto-scaling-benefits.html) |
| DynamoDBは条件付きtransactionを提供する | 両案の受付・terminal原子更新を実装する土台。schemaと競合処理は未検証 | [transactions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html) |
| 対応EC2はnested virtualizationを提供する | 両案のKVM利用候補になる。Firecracker統合・隔離・公平性は別途検証 | [EC2](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/amazon-ec2-nested-virtualization.html) |

## 実測の範囲

**A・Bを同じ条件で構築して比較したデータはない。**

| 比較対象 | 保存済みevidence | この2案について言えること |
| --- | --- | --- |
| coldからの応答時間 | [公平性実験](../fairness-experiment.md)、[PG pool実験](../postgres-pool-experiment.md) | 別構成・別bootstrapの値。A/Bの起動時間や10秒適合へ転用できない |
| 公平性・TLE境界 | [公平性データ](../fairness-data/)、[環境](../fairness-environment.json) | ARM64 / gVisor等の条件。今回のEC2＋Firecrackerの適合証明ではない |
| worker死亡・応答消失・terminal競合 | [PG poolの試験範囲と失敗](../postgres-pool-experiment.md) | 別実装。A/Bの回復手順は未実証 |
| 費用・無提出時固定費 | [取得済みPrice List](../production-judge-prices.json) | 単価原票の一部。両案の全サービス・利用量を揃えた見積は未作成 |
| 実装量・保守負担・進捗遅延 | 比較実測なし | 優劣を断定する根拠はない |

現行の統合設計は[構成書](../aws-judge-orchestration.md)に置く。
Bの原提案にないSQS終端失敗配送は、その構成書で定義した追加責務。
