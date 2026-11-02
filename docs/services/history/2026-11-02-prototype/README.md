# 過去の試作・測定記録（現行設計ではない）

このディレクトリは `c66c0c6aebaadb9bbecf2360a3493f0de070bd8b` までの試作の記録。**現行の1 VM・同時1実行のpolicyや、そのscalingの実証と混同しない。**

当時のPoolはASG最小0・最大1台、VM内2実行だった。動画は実AWS・実native実行の記録であり、確認したのは1台の中での並行実行とJSON pollingによる表示。複数VMへのautoscalingや将来需要の先読みを実証した動画ではない。以前の「構成を固めた」という報告はその範囲を超えていた。

- `serverless-judge-e2e.json` / `serverless-judge-late-result.json`: 先行のper-node Fargate案。
- `pool-judge-*.json`: 当時の1台・2実行Poolの試験。7ノードのwarm P95 8.511秒はこの条件の値。
- `demos/`: 当時の並行動画とsnapshot pollingの観測。SSE/NDJSON購読はない。
- `diagrams/`: 当時の構成図・配布ライセンス。
- 古い設計の説明文は現行ツリーから削除した。[当時の構成説明](https://github.com/traP-jp/traOJudge/blob/c66c0c6aebaadb9bbecf2360a3493f0de070bd8b/docs/services/serverless-judge.md)、[当時の関心事](https://github.com/traP-jp/traOJudge/blob/c66c0c6aebaadb9bbecf2360a3493f0de070bd8b/docs/services/serverless-judge-concerns.md)、[当時のデモ説明](https://github.com/traP-jp/traOJudge/blob/c66c0c6aebaadb9bbecf2360a3493f0de070bd8b/docs/services/judge-flow-demo.md)はcommit参照のみとする。

不要になった実行コード（Fargate stack/dispatcher、online worker、旧デモ・旧runner）は現行ツリーから削除した。過去の再現が必要なときだけ、[当時のソース](https://github.com/traP-jp/traOJudge/tree/c66c0c6aebaadb9bbecf2360a3493f0de070bd8b/infra)を参照する。
