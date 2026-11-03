# 独立提案A：固定Step Functions＋DSL interpreter＋SQS callback

2026-11-03 JST。GPT-6 Astra / Maxによる独立した構成提案。
既存リポジトリ、過去の設計、他の提案を参照せず、公開一次資料だけで策定。
本レポートはその提案を整理したもの。コード編集・deploy・実測は行っていない。

## 受領した条件

AWS内のproduction向けオンラインジャッジ。ordering、durable execution、isolated execution、
real-time進捗表示、EC2最大5台を扱う。
途中で、提出順が守られるが実行順は自由、作問者がフローをデータとして登録する、
問題ごとのリポジトリ・IaC変更は不要、物理的な再実行は禁止されない、と明確化された。
consumer間の排他・再配送、最終判定1回またはDLQ、再起動・master死亡・worker死亡を説明する。

**EC2最小2台は提案者の可用性上の選択であり、ユーザーの必須条件ではない。**

## 提案

**固定のStep Functions Standardが作問者のDSLを解釈し、SQS callbackでEC2 workerへ配送する。**

| 責務 | 担当 |
| --- | --- |
| 登録・受付・状態取得 | API Gateway / Lambda |
| 受理順・冪等性・node状態・lease・terminal・outbox | DynamoDB |
| フロー・ソース・問題データ・成果物 | S3 |
| 汎用フローの実行位置・待機・retry | Step Functions Standard |
| 実行可能nodeの計画・依存解決 | 独自DSL interpreter / PlanNext |
| 実行依頼と再配送 | SQS Standard＋worker long polling |
| node単位の配送失敗 | SQS transport DLQ |
| judgement全体の終端失敗 | SQS judgement DLQ＋DynamoDB失敗台帳 |
| 隔離実行 | EC2 ASG＋Firecracker |
| 進捗通知 | DynamoDB Streams＋publisher＋AppSync Events |
| 未処理outbox・異常終了の修復 | EventBridge / Lambda / CloudWatch |

## 作問者のフロー

版付きJSON等に、node、依存関係、条件分岐、上限付き並列処理、プログラム・入力参照を定義する。
初期範囲は条件分岐を持つ有限DAG。反復を追加するなら回数・時間を制限する。
checker・generatorはS3のプログラムを参照し、sandbox内で実行する。

固定workflowは次を繰り返す。

`フロー読込 → PlanNext → 実行依頼・callback待機 → node結果保存 → PlanNext → 最終判定`

judgement開始時に、フロー・問題データ・コンパイラ・checkerの版を固定する。

DSLの構文検証・解釈・依存管理は独自実装。
問題別state machineの作成は不要で、任意ASLを入力から動的実行する構成ではない。
Step Functionsは固定エンジンの状態を保持し、SQS callbackでworkerの完了を待つ。[S1][S2]
大きな状態はS3 / DynamoDBへ置き、256 KiBの入出力・25,000件の実行履歴上限を考慮する。[S3]

## 受付とconsumer間の排他

この提案では提出順を受付commit時に確定する連番と解釈する。
DynamoDB transactionでcounterの条件付き更新、`submissionSeq`付き提出、起動outbox、
冪等キーをまとめて保存する。counter競合は読直し・再試行し、順番はcommit時に確定する。[S4][S5]
起動relayの脱落は定期照合で修復する。固定name・inputによる起動と永続台帳を併用する。[S6]

workerは空き枠があるときに同じSQS queueをlong pollingする。
SQS Standardは重複配送を許容するため、visibility timeoutを排他ロックとは扱わない。[S7][S8]
nodeごとに次を保存し、条件付き更新で所有権を取得する。

`judgementId / nodeId / attemptId / owner / leaseExpiresAt / fencingEpoch`

実行中はleaseとvisibilityを更新し、引継ぎではepochを増やす。
古いepochの進捗・成果物採用・結果保存は拒否する。
分断中の旧workerの物理実行までは排除せず、正規状態への反映を拒否する。
callback待ちは有限期限にし、キュー待ちにも進む短いheartbeatをworker死亡判定に流用しない。

## 最終判定とDLQ

成果物をS3へ保存し、node結果をDynamoDBへ確定してからcallbackする。
再配送時は保存済み結果を再利用する。AC / WA / CE / TLE等は正常な採点結果。

judgementの未終端条件を付け、一度だけterminalを確定する。
retry予算を使い切った場合は、失敗終端とjudgement DLQ向けoutboxを同じtransactionで保存する。
配送未完了は再送し、message重複はjudgement IDで扱う。DynamoDBとSQSの同時原子commitは前提にしない。

transport DLQはnode配送の失敗であり、bridgeがworkflowへ失敗通知する。
フローのretry / fallbackと、judgement全体の失敗確定は区別する。
SQS保持は最大14日なので、失敗履歴・再実行入力はDynamoDB / S3にも残す。[S9]

## 停止と回復

| 状況 | 回復手段 |
| --- | --- |
| 受理commit後、起動前にrelay停止 | 起動outboxの再送・定期照合 |
| workflow起動成功後に応答消失 | 固定name・inputで既存executionを確認 |
| PlanNext Lambda停止 | 冪等な計画・node操作をStep Functionsから再試行 |
| workerが結果保存前に停止 | visibility / lease満了後、新epochで引継ぎ |
| 結果保存後、callback前に停止 | 保存済み結果でcallbackを再送 |
| callback後、SQS削除前に停止 | 再配送時に保存状態を確認し、再採点せず終了 |
| 旧worker復帰 | epochとterminal条件で古い結果を拒否 |
| workflowの未処理例外・全体timeout | Reconcilerが保存結果の回収または失敗終端 |
| DLQ送信中にrelay停止 | 未完了outboxを再送 |
| 全worker再起動 | SQS / DynamoDB / S3 / Step Functionsから再開 |

常駐masterは置かない。eventual終端はAWSの復旧と修復処理の再開を前提とする。

## 隔離・台数・進捗

提案者の開始構成は2 AZ・EC2 2台、上限5台。ASGは滞留・枠利用率で拡縮し、処理中はscale-in保護とdrainを行う。[S10]
置換中も実台数5台を厳守するなら、MaxSizeだけでなくAZ rebalancing等の上限超過も制約する。[S11]

提出・checker・generator・compileはfresh microVM内で実行する。
guestにNIC、AWS資格情報、task token、host filesystemを渡さない。
CPU・memory・wall・output・diskを制限し、jailer / seccomp / cgroup / kernelを保守する。[S12]
比較用正解データと判定確定処理は信頼する側に置く。
対応EC2のnested KVMを使う案であり、採点時間の再現性は実測する。[S13]

進捗は保存後にAppSync Eventsへ配信し、判定ID・node ID・revisionで重複・旧状態を除外する。
再接続時のsnapshotと低頻度再取得で復元する。[S14]

## 未検証

DSL構文と上限、受付counterの競合、KVM / Firecracker統合、公平性と資源制限、
lease / visibility / retry / deadlineの具体値、応答消失や旧worker復帰を含む障害注入、
厳密な台数制御、費用、quota、進捗遅延は未検証。

## 一次資料

- [S1: Standard workflow](https://docs.aws.amazon.com/step-functions/latest/dg/choosing-workflow-type.html)
- [S2: SQS callback連携](https://docs.aws.amazon.com/step-functions/latest/dg/connect-sqs.html)
- [S3: Step Functions上限・設計指針](https://docs.aws.amazon.com/step-functions/latest/dg/sfn-best-practices.html)
- [S4: DynamoDB transactions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html)
- [S5: Transactional outbox](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/transactional-outbox.html)
- [S6: StartExecution](https://docs.aws.amazon.com/step-functions/latest/apireference/API_StartExecution.html)
- [S7: SQS重複配送](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/standard-queues-at-least-once-delivery.html)
- [S8: Visibility timeout](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/sqs-visibility-timeout.html)
- [S9: SQS保持期間](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/creating-sqs-fifo-queues.html)
- [S10: SQSに基づくAuto Scaling](https://docs.aws.amazon.com/autoscaling/ec2/userguide/as-using-sqs-queue.html)
- [S11: ASGのrebalancing](https://docs.aws.amazon.com/autoscaling/ec2/userguide/auto-scaling-benefits.html)
- [S12: Firecracker host設定](https://github.com/firecracker-microvm/firecracker/blob/main/docs/prod-host-setup.md)
- [S13: EC2 nested virtualization](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/amazon-ec2-nested-virtualization.html)
- [S14: AppSync Events](https://docs.aws.amazon.com/appsync/latest/eventapi/event-api-welcome.html)
