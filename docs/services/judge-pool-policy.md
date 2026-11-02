# VM poolの設計と変更するパラメータ

この文書は**設計を議論するための参照先**。[質問への回答](serverless-judge.md)と[実験で確認した範囲](scaling-experiment.md)は別にまとめている。ここに安全条件を定義したことと、全故障下で実証したことは区別する。

caseの非ACは判定jobの失敗として既存Engineへ通知する。同じpipelineの新規dispatchを止めるのはEngineであり、poolやworkerに別の枝刈り処理を持たせない。[木構造デモ](tree-demo.md)で発行済み実行と新規dispatchの区別を確認できる。

## 先に決めていること

| 判断 | 現在の選択 | 意味・代償 |
| --- | --- | --- |
| 実行の配置 | 1 VM・同時1ノード。通常pool上限5台。 | 同じVMに複数の実行を同時配置しない。超過需要はqueueで待つ。 |
| VMの再利用 | 退役前のVMはcleanup後に再利用。 | 毎ノード新規EC2の方式ではない。cleanup失敗時は以後使わない。 |
| 将来需要 | 既知の木が全て通過する場合の240秒内peakを先読み。 | 時刻別の容量予約や、起動時間に応じた供給時刻の保証はない。 |
| VMの退役 | 新規受信を止め、既存処理を終えてから破棄。決定後は再開しない。 | 遅れて届くscale-inに対する安全条件。その間の新規提出は代替VM待ちになり得る。 |
| 異常なVM | 保護を維持し、新規実行・退役の完了報告を止める。 | 安全側に停止するが、自動交換がなく処理と課金容量が滞留する。 |
| 配置・置換 | 単一AZ。置換では先に終了し、その後起動。 | 通常の上限を守る方向の設定。可用性・交換速度を犠牲にする実験条件。 |
| 費用制御 | poolの通常上限を5台に制限。 | 請求総額の上限ではない。受付量・保存量・稼働期限・予算停止が別途必要。 |

## どこを変更するか

[pool.json](../../infra/policy/pool.json)が設定の正本。[schema](../../infra/policy/pool.schema.json)が形式、[policy.py](../../infra/runtime/policy.py)が項目間の制約と純粋な台数・退役判定を定義する。判定関数はAWSに依存しないため、入力と出力だけで方針を議論・検証できる。

CDKはsynth時に選んだprofileを検証し、同じcompiled JSONを`POOL_POLICY`としてcontrollerとhostへ渡す。CloudFormationの出力にprofile名とSHA-256 fingerprintを残す。実行中に台数上限や方式を任意変更するAPIは持たせない。変更はpolicy文書を選んだCloudFormationデプロイで行う。

| profile | 需要計算 | warm数 | 共通上限 |
| --- | --- | --- | --- |
| `lookahead`（既定） | 待ち・実行中＋既知の将来の枝。 | 1 | 5 |
| `reactive` | 待ち・実行中。先読みの並列数を使わない。 | 1 | 5 |
| `prewarmed` | `lookahead`と同じ。 | 5 | 5 |

`--context poolProfile=<name>`でprofile、`--context poolPolicy=<path>`で定義ファイルを選ぶ。以前の`poolMax`、`nodeVisibility`、`nodeAttempts`、`idleSeconds`の個別overrideは拒否する。

## 需要から台数を決める

1. NodeQueueの待ち・受信中の件数と、graph上のready/starting/running件数を取る。同じ仕事を二重に数えないよう最大値を使い、hostの実行中件数も下回らないようにする。
2. `lookahead`では各runの既知の依存graphを辿り、全gateが通過する場合のpeakを加味する。各ノードの所要時間にはwall制限の全量を使う。実行済みノードは残り時間0、失敗・cancelledの祖先に続く仕事は除外する。
3. warm数と制御処理の待ちも加味し、必要台数をprofileの`operating_limit`以下に制限する。ASGの最大値も5に宣言する。
4. 増設は台数metricを発行する。縮小は下記の退役確認が済むまで待つ。

先読みの出力は期間内の**最大台数**であり、各時刻の供給予約ではない。親が早く終わるとモデル上期間外だった枝が先に現れるため、需要の厳密な上界でもない。複数runのpeakを足すと過大にもなる。未知の提出・gateの成否・EC2の起動時間は予測していない。

需要記録の期限は300秒。仕事の状態変化で更新するが、checkpointを定期走査して失われた更新を再構築する処理はない。古いqueue統計と更新欠落が重なると需要を過小評価し得る。DynamoDB全走査・S3のVM別状態報告も本番高負荷向けのindexではない。

## 台数を計算するコードと、増減するAWSを分ける

自作controllerは`SafeDesiredInstances`を発行する。**必要台数の計算自体は自作コード**であり、managed schedulerへ置き換わったわけではない。

ASGを変更するのは、CloudFormationに宣言したCloudWatch alarmとExactCapacity StepScaling policy。非zero用のpolicyに1〜5台、別のpolicyに0台を定義する。controllerのIAM roleには`SetDesiredCapacity`や`UpdateAutoScalingGroup`を与えない。

metricは60秒のMaximum、1期間で評価し、欠測は`NOT_BREACHING`。状態変化時と1分ごとの再確認で発行する。欠測を0台の要求として扱わない。一方、過去に出した低い値に基づく、既に許可された退役まで取り消すわけではない。過去の高い値や遅延したalarm actionは収束を遅らせる。

推定instance warmupは0秒に設定しているが、実VMの起動時間が0になる意味ではない。StepScalingにはSimpleScalingのcooldownは適用されない。次の退役前待ち時間60秒はcontroller自身の別の条件である。

## 通常scale-inで実行を壊さない条件

1. **起動時から保護する。** 新規VMはASGのscale-in保護付きで起動する。
2. **退役を永続保存する。** controllerはVMごとの退役tokenを保存してから指示する。hostもdiskへ保存し、agent再起動後も退役を撤回しない。
3. **新しい受信を止める。** 受信予約と退役は同じlockを使う。指示前から待機中だった受信が1要求を返した場合、その処理は保護したまま実行する。単に受信をやり直してretry回数を消費しない。
4. **受信・実行・配送・cleanupを終える。** hostは全て終わるまで保護を解除しない。実行0だけでなく受信待ちも0である必要がある。
5. **controllerが独立に確認する。** 新鮮な`drained`報告、token一致、`active == 0`、明示的な`polling is False`、ASG上の保護解除を確認した分だけ台数metricを下げる。不明・古い・起動途中・隔離されたVMは退役完了として数えない。

確認できるVMがなければ現在容量を保持する。保護をかけ直してもAWSが既に選んだ回収を撤回できないため、**退役VMはresumeしない**。新規需要が来ても退役を完了し、その後に代替VMを要求する。この安全条件とcold待ち時間の交換条件は[実験レポートの質問5](scaling-experiment.md)で確認できる。

ASGの通常scale-in以外、たとえばEC2障害・health replacement・外部強制停止・stack削除は保護で防げない。それらは次の再試行で扱う。

## 再利用・再試行・DLQ

hostはsandboxと作業領域の削除を検証・記録してから次のノードへ進む。agent再起動時も前のsandboxと作業領域を掃除する。cleanup失敗では以後の実行を止め、状態報告の通信異常では新しい受信予約を止める。cleanup失敗は隔離状態となり、退役完了も報告しない。

NodeQueueはvisibility 180秒、更新30秒、受信上限5回。結果を永続保存してから通知・要求ACKする。保存後の再配信は既存結果を利用する。保存前の停止は再実行され得るため、exactly-once実行ではない。結果の条件付き保存と試行ごとのartifactパスで競合を区別する。

受信上限後はNodeDLQへ移り、元要求を隔離保存して基盤障害に終端化する。明示再判定は新run。ControlDLQもあるが、そこからの回復と受付streamの枯渇後回復は未完成。

boot-stuck、agent死亡、cleanup隔離を検出して**自動交換する経路は未実装**。保護を維持したVMが容量・費用を占有し続ける。安全に新規実行を止める条件と、サービスを復旧して進める条件を区別して追加設計する必要がある。

## warm維持と管理用停止

初期状態では、通常profileは明示的に`sleep`するまで1台warmを保持する。`idle_seconds = 900`があるだけで、初回から常に15分で自動zeroになるわけではない。sleep後の次の受付は`wake`でactivityを更新し、その後15分のwarm維持が有効になる。`prewarmed`は同じ条件で5台を保持する。

管理actionは`reconcile`、`wake`、`sleep`のみ。sleepは観測した待ち・受信中・実行中・制御待ち・予測仕事があれば拒否する。新しい受付とshutdownは同一transactionではないため、運用では先に受付を止める必要がある。

## 主要な値と未決事項

| 正本の項目 | 現在値 | 用途 |
| --- | --- | --- |
| `limits` | 最大5、VMあたり同時1。 | 通常の容量上限。 |
| `queue` | visibility 180秒、更新30秒、上限5回。 | NodeQueueの再配信とDLQ。 |
| `host` | 状態報告3秒、古さ20秒、受信待ち10秒、container上限90秒。 | hostの生存観測と受信・実行。 |
| `scaling` | 先読み240秒、idle 900秒、縮小前待ち60秒、metric周期60秒、需要期限300秒、controller lease 90秒。 | 台数計算と縮小の待機。 |
| `placement` | AZ 1、置換時のhealthy最小0%・最大100%。 | 通常の置換で一時的な増台を避ける配置条件。 |

項目間の制約も検証する。たとえばvisibilityはcontainer上限より長くし、visibility内に少なくとも2回の更新機会を確保する。[check-pool-policy.py](../../infra/scripts/check-pool-policy.py)はAWSを呼ばず、profile・先読み・上限・退役token・受信予約・再起動の境界を確認する。実際のAWS増減の証拠は[実験レポート](scaling-experiment.md)へ分ける。

次の判断が残る：供給に間に合う予測方式、退役中の需要回復の待ち時間、不良VMの交換、ControlDLQ回復、受付公平性・quota、保存量・保持・稼働期限・金額予算。[関心事一覧](serverless-judge-concerns.md)で追跡する。
