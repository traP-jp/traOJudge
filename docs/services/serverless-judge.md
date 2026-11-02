# ジャッジ実行基盤の入口

現行の制約は **1 VM・同時1実行、最大5 VM**。複数実行を1台に詰め込む経路と、固定0/1台の容量制御は削除した。

- [議論するための独立した pool policy](judge-pool-policy.md)
- [policy 定義](../../infra/policy/pool.json)・[schema](../../infra/policy/pool.schema.json)
- [実 AWS の scaling 実験レポート](scaling-experiment.md)・[生の観測記録](scaling-experiment.json)
- [現在の関心事・不足](serverless-judge-concerns.md)
- [現行の構成図（PNG）](diagrams/judge-pool.png)・[自己完結HTML](diagrams/judge-pool.html)（[caru-ini/stack-diagram](https://github.com/caru-ini/stack-diagram)で生成）

policy はデプロイ時に選択・検証し、同じ定義を容量計算と実行ホストへ渡す。アプリケーションが都度台数を設定する構成ではない。ASG、CloudWatch Alarm、ExactCapacity Scaling Policy、初期 scale-in protection、上限と配置・置換条件を CloudFormation で宣言する。

実験用の入力は `infra/experiments/` に置く。C17 のケース木、意図した失敗、待機時間は実験入力であり、製品の問題登録契約や scaling policy へハードコードしない。

[以前の試作とその実測](history/2026-11-02-prototype/README.md)は履歴へ分離した。そこでの2slot、0/1台、7ノードのP95、逐次／並行デモは現行構成の測定値ではない。過去の測定を変更せず、当時のcommitと条件を残す。
