# Judge構成の採用判定

現案の採用判定は保留である。各関心事の規則と未確定事項は、以下の文書に置く。規則の矛盾は外部サービスを動かさずに審査し、隔離や性能の成立には対象基盤の実測を使う。

- [受付順](judge-contracts/submission-order.md)
- [判定の終端確定](judge-contracts/judgement-completion.md)
- [未信頼コードの実行隔離](judge-contracts/execution-isolation.md)
- [進捗と結果表示](judge-contracts/progress-results.md)
- [作問者定義と問題版](judge-contracts/author-workflows.md)
- [判定時間の公平性](judge-contracts/fairness.md)
- [起動台数と10秒応答](judge-contracts/capacity-latency.md)
- [無提出時の費用と利用量に応じた費用](judge-contracts/cost.md)

## AWSへの割り当て

| 処理 | 割り当てるサービス |
| --- | --- |
| 受付順、起動待ち、最終結果、配送待ち、表示用状態 | DynamoDBのトランザクションと永続台帳 |
| 作問者フローの実行、分岐、待機、再試行 | Step Functions Standard |
| 実行待ち、配置、ワーカー障害時の再試行 | AWS Batchのキューと配置処理 |
| 提出、コンパイラ、採点プログラムの隔離 | EC2上のFirecracker microVM |
| 不変な入力と成果物 | S3 |
| 最終失敗の配送 | SQS |
| 進捗通知 | DynamoDB StreamsとAppSync Events |
| 起動漏れ、期限超過、未配送、残存ジョブの回収 | 永続台帳を読む定期照合処理 |

この割り当ては実装候補であり、各契約が成立する証拠ではない。

![判定規則とAWSへの割り当て](diagrams/aws-judge-orchestration.png)

[HTML版](diagrams/aws-judge-orchestration.html)・[暗色版](diagrams/aws-judge-orchestration-dark.png)

[独立提案と比較、実測資料](judge-evidence.md)は採用判定の根拠として参照する。
