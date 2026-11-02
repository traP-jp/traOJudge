# PostgreSQL queue / EC2 pool の最小実証

2026-11-02。仮置きの実験であり、運用構成の確定ではない。PRは作成しない。
AWS実機での確認を進めている段階で、以下の図は適合済みを意味しない。

![PostgreSQL / EC2 pool](diagrams/postgres-pool.png)

[操作可能なHTML](diagrams/postgres-pool.html) / [dark PNG](diagrams/postgres-pool-dark.png)。
図は [caru-ini/stack-diagram](https://github.com/caru-ini/stack-diagram) のtemplate・icons・build/rendererで生成。

- 分散単位は既存workflow Engineのcompile → run → checkノード。
- PostgreSQLに提出の受理順、DAG、実行リース、結果、DLQを保存。`SKIP LOCKED`で取得し、古い実行の結果はtokenと期限で拒否する。
- EC2 Auto Scaling Groupは0〜5台。1台に同時1ノード、制御はCPU 0、sandboxはCPU 1。c7i.xlargeのSMTを無効にする。
- Rustが固定Nix環境をloadし、毎ノード新しいgVisor sandboxを作成してcleanupする。worker停止時は独立したhost watchdogが残存sandboxを回収する。
- Lambdaがpool外で受付・復旧・起動を担う。1分tickとAuto Scaling API用PrivateLinkを用意し、EC2が0台でも復旧を続ける。

ローカルの実PostgreSQLで、5並行claim、再取得と古い結果の拒否、3失敗後のDLQ、実DAGを逆順に完了した際の公開順、期限切れと復旧の競合を4テストで確認した。
固定Nix環境ではGCC 15.2.0、Python 3.13.12、Rust 1.94.1のhello fixtureを実行できた。

未確定の点:

- EC2 cold startを含む10秒目標。従来のEC2実測では未達であり、今回もwarm時と分けて測る。
- host cgroupのCPU時間はgVisor Sentryの処理も含む。guestの粗いCPU clockを避けられても、公平性の合格とはしない。
- 今回のRDSはSingle-AZ、backupなしの使い捨て設定。運用時の耐障害性はMulti-AZ・backupを含め別評価する。
- 実証ではサイズ制限付きartifactもPGに保存する。運用時のS3移管・保存期限・DB容量の設計は残る。
- scalerの受付との競合、負荷時の受付制限、fairnessの受入閾値。

実機結果、費用概算、ordering / durable / isolated executionのチェックリストを追記する。
