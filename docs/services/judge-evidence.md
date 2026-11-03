# Judge構成の根拠資料

## 成果物

- [現行構成と障害回復の契約](aws-judge-orchestration.md)
- [独立提案A：固定workflow＋DSL＋SQS](judge-proposals/sqs-callback.md)
- [独立提案B：作問者ASL＋Batch](judge-proposals/native-asl-batch.md)
- [比較：提案・サービス仕様・実測の範囲](judge-proposals/comparison.md)

## 実測と再現手順

| 記録 | evidence | 確認した範囲 |
| --- | --- | --- |
| [Lambda疎通](lambda-smoke.md) | [呼出しと削除](lambda-smoke-evidence.json) | CDK作成・異なる入力2件の応答・撤去。judge適合試験ではない |
| [公平性・cold応答](fairness-experiment.md) | [生データ](fairness-data/) / [集計](fairness-summary.json) / [環境](fairness-environment.json) / [検査](fairness-data-validation.json) / [撤去](fairness-teardown.json) | ARM64 MicroVM・EC2＋gVisor・native対照。今回の2案の比較実験ではない |
| [PostgreSQL queue / EC2 pool](postgres-pool-experiment.md) | [集計](postgres-pool-data/summary.json) / [event例](postgres-pool-data/event-example.json) / [撤去](postgres-pool-data/teardown.json) | 旧実験の観測結果と未達事項。現行構成の実装ではない |
| 料金原票 | [Price List](production-judge-prices.json) / [RDS compute](postgres-pool-data/multi-az-prices.json) / [storage](postgres-pool-data/storage-prices.json) | 取得時点の単価。現行構成の月額見積ではない |

各実験レポートから、当時のCDK、fixture、collector、再現手順へ辿れる。
これらのコードは測定条件を確認・再現するための資料。
現行構成の採用実装と混同しない。

サービス仕様の一次資料は各独立レポートと比較表に対応付ける。
仕様・設計提案・実測結果を同じ確度の証拠として扱わない。
