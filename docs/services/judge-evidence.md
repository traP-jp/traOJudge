# Judge構成と実測資料

## 構成と提案

- [現行構成](aws-judge-orchestration.md)
- [独立提案・固定ワークフローとSQS callback](judge-proposals/sqs-callback.md)
- [独立提案・作問者ASLとAWS Batch](judge-proposals/native-asl-batch.md)
- [二つの独立提案の比較](judge-proposals/comparison.md)

## 実測

| レポート | 保存した結果 |
| --- | --- |
| [Lambda疎通](lambda-smoke.md) | [呼び出しと撤去](lambda-smoke-evidence.json) |
| [公平性と応答時間](fairness-experiment.md) | [生データ](fairness-data/)、[集計](fairness-summary.json)、[測定環境](fairness-environment.json)、[データ検査](fairness-data-validation.json)、[撤去](fairness-teardown.json) |
| [PostgreSQLのキューとEC2プール](postgres-pool-experiment.md) | [集計](postgres-pool-data/summary.json)、[イベント例](postgres-pool-data/event-example.json)、[撤去](postgres-pool-data/teardown.json) |
| 取得時点の料金原票 | [AWS Price List](production-judge-prices.json)、[RDS compute](postgres-pool-data/multi-az-prices.json)、[RDS storage](postgres-pool-data/storage-prices.json) |
