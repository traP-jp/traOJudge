# 1 VM・同時1実行の scaling 実験

2026-11-02、東京リージョンの専用 CloudFormation stack で、実 Engine・native C17 実行・gVisor・S3 artifact・SQS を使って測定した。**通常の健全なホストでの宣言的な増減、退役、再試行は確認したが、本番受入は未達**。起動不良・cleanup quarantine の自動交換、ControlDLQ の回復、将来需要に対する供給時間の保証が残る。通常10秒以内という目標も達成したとは判定しない。

## 議論する設計の正本

台数・同時実行数・先読み期間・retry・heartbeat・退役待ち・配置／置換条件は [pool.json](../../infra/policy/pool.json) に集約した。[schema](../../infra/policy/pool.schema.json) と [純粋な policy 関数](../../infra/runtime/policy.py) で検証し、CDK が同じ compiled JSON と fingerprint を容量 adapter と host に渡す。[設計の説明](judge-pool-policy.md) は実測とは分けて読める。

fresh subagent には過去の会話を渡さず、現行コードと条件だけを渡した。旧0/1台固定・VM内2slot・Fargate dispatcher・online worker・旧デモの実行経路を削除した。以前の動画・図・測定は [履歴](history/2026-11-02-prototype/README.md) に当時の条件とcommitを付けて隔離した。過去の warm P95 を現行構成の数値として使わない。

現行の制約は、VMあたり同時に1ノード、通常のpool上限5台。各ノードを新しいoffline gVisor sandboxで実行し、containerと作業領域の削除を検証してから同じVMを次のノードに使う。**1ノードごとに新しいEC2を起動する契約ではない**。退役を決定したVMは再開せず、完全に回収してから代替VMを使う。

容量 adapter は `SafeDesiredInstances` metric を出す。希望台数を変更するのは CloudFormation に定義した CloudWatch alarm と2つの ExactCapacity ASG policy。adapter に `SetDesiredCapacity` 権限はない。ASG最大5、起動時のscale-in protection、単一AZ、置換0/100%を宣言した。単一AZ・terminate-before-launchは可用性より通常の台数制約を優先する実験条件であり、本番の推奨配置を意味しない。

## 試験A：木・再利用・VM喪失・DLQ

[観測JSON](scaling-experiment.json) と [runner](../../infra/experiments/run_scaling.py)。profileは `lookahead`、fingerprintは `83496c57063b54b19a470ddf32b47a3dd346ca915ac3f84c2cd367a73b2a0e60`。この試験は後述のpoll予約修正**前**のソースで実施した。JSONの失敗判定を後のソースの成功として書き換えない。

| 条件 | 実測・判定 |
| --- | --- |
| 8段のgate後に5枝、13ケース・28ノード | AC。最後のdurable checkpoint公開まで174.623秒。保存された実行区間の最大並行は5。 |
| 将来の5枝への準備 | 5台readyの初観測は最初の枝の実行開始より0.900秒後。約5秒の観測間隔を考慮しても、先行準備を実証したとは言えない。 |
| 親WAの木、独立した別の根あり | 21.216秒、WA。実行済み3・SKIP3。SKIP子孫のrun/checkにはattempt記録がなく、独立した根は継続。 |
| 連続提出A/B | 4.756秒／10.830秒。同じ物理VMをcleanup検証後に再利用。sentinelと子processを含むfixtureで前のsandboxが残らないことも確認。 |
| native実行中にhostを実際にpoweroff | 同じrunが228.783秒でACへ復旧。停止時のDocker実行中を記録。失われたノードの受信回数記録は1、4で、2、3の欠落原因は記録だけでは確定できない。 |
| 結果保存後・通知/ACK前にpoweroff | 同じrunが186.684秒でACへ復旧。保存済み結果を再利用し、対象compile artifactを再生成していない。 |
| 常に失敗する要求 | 受信1〜5回でNodeDLQへ移動。元要求を隔離保存し、基盤障害としてIEに終端化（10.395秒）。 |
| IE後の明示再判定 | 同じsubmission・新しいrunでAC（9.723秒）。無限再試行にしない。 |
| 不可逆な退役中の新規提出 | 87.110秒でAC。退役したbootをresumeせず、安全な代替VMで処理。 |
| 終了時 | ASG希望0・所属VM0。NodeDLQ／ControlDLQのvisible・inflight・delayedを2回取得し全て0。 |

時間は受付からrunnerが終端を観測するまで（先頭の木だけdurable checkpoint公開時刻）。復旧時間にはvisibility 180秒、EC2 health replacement、alarm周期が含まれる。at-least-onceと上限付きretryを使うので、恒久障害でも必ずACになる保証やexactly-once実行の保証はない。

実際のASG activityには、alarm/policyを原因とする1→5、5→1、最終1→0と、強制停止に伴うhealth replacementが残った。試験全体では11個の物理VMを識別し、全てterminatedを確認した。公開記録のVM名は照合用opaque labelであり、AWSの実IDではない。

### 予測と回収で未達だったこと

需要予測は「未確定gateが全て通る」「各ノードがwall制限だけかかる」と仮定した240秒のearliest-start overlap。新しい提出やgateの成否を予知しない。gateが想定より早く終わると、モデル上horizon外だった枝が先に現れるため、**horizon内需要の厳密な上界でもない**。複数runのpeakを足すと過剰にもなり得る。

木の受付後、約81秒で希望台数5、約92秒で所属VM5、約172秒で5台readyを初観測した。初期image取得等も含む。今回はcold供給が広い層に間に合ったことを証明できなかった。将来の需要を表せることと、その時刻にVMが使えることは別の受入条件である。

また、短時間の予測3が撤回された後も、遅延した60秒Maximumのalarmと、退役待ち／cooldown中に現在容量を再発行する制御により、1→3などの余剰増減が残った。関連する観測ではraw需要・targetは1でもpublished safe targetが3や2となっている。SQSの近似値が原因だったとは断定しない。この保守的なfeedbackは、時間・費用効率の未解決事項として残す。

## 試験B：long poll と退役の競合

fresh reviewで、`active == 0` でも `ReceiveMessage` が待機中なら受信の権利が残っていることを確認した。旧コードではその間に保護を外し、返ってきた要求をvisibility 0で戻してretry budgetを消費し得た。試験Aの欠落した受信回数の原因をこの競合だと断定する証拠はない。

修正では、poll予約とdrainが同じlockを使う。予約済みのreceiveは、drain後に1件受信した場合も保護されたまま実行・結果配送・cleanupまで完了できる。新しいpollは開始しない。controllerは新鮮な退役tokenの一致、`active == 0`、明示的な`polling is False`、ASGの保護解除を別々に確認し、それまではmetricを下げない。不明なVMしかいない場合も希望容量を保持する。純粋なpolicy/hostテスト15件でこの境界を検証した。

実環境の追加試験は [専用runner](../../infra/experiments/run_drain.py) と [別の観測JSON](drain-experiment.json) を使い、同じ定義の `prewarmed` profile（5台warm）で実施した。poll中の退役を観測してから5提出を投入した。30ノードのattemptは全て受信回数1、うちcompileの1件は`reserved_before_drain`であり、対象のinterleavingから実際のnative実行・配送・cleanupまで通った。5提出とも2ケースACとなり、元の退役bootは既存のpoll予約以外から新規実行を開始していない。保存済み区間の物理VM内重複もなかった。追加の重複配信は行っていない。

この試験のprofileは`prewarmed`、fingerprintは`0efdf8e7b65bac66a99a079443e97a1ed67e7ca9b241079b959477a2169f2aad`。観測JSONにデプロイ対象ソース各ファイルのSHA-256も記録した。保存済み区間の最大並行は3であり、この試験だけで5並行を主張しない。初期5台と代替5台の計10 VMは、全てterminatedを照合した。

5台warmは起動済みの初期条件であって、退役後に5台をそのまま再利用する条件ではない。今回は全台を不可逆に退役させた直後の提出なので、代替VMのcold起動とalarmを待ち、通常10秒以内には収まらなかった。各提出の最後のdurable checkpoint公開まで283.763〜285.670秒。runnerが提出を順番に観測した終端時間294.793〜354.124秒には、後続提出の確認が遅れた時間も含まれる。観測の遅れを実行時間そのものやP95として解釈しない。

## 回収・再試行の保証範囲

- 通常scale-inでは、新規VMを最初から保護する。drain tokenをdurableに保存し、hostがそれをdiskにも保存し、poll・実行・配送・cleanupを終えるまで解除しない。保護を戻しても既に選ばれた回収を撤回できないため、退役VMをresumeしない。
- 実行中のEC2障害・外部強制停止・stack削除はscale-in protectionでは防げない。SQS visibilityの更新、durable結果、上限後のNodeDLQによって復旧または明示的な基盤障害へ進める。
- NodeDLQは隔離保存後にrunをIEにする。ControlDLQとreceipt streamの枯渇後回復は未実装。単にDLQがあることを回復保証とは呼ばない。
- boot-stuck、agent死亡、cleanup失敗は保護を維持してfail-closedになる。**自動health交換がないため、処理と課金容量が滞留する本番blocker**。検証せずに保護を外すwatchdogを追加して解決扱いにしない。
- 管理用sleepは観測した待ち／実行／control／予測仕事があれば拒否する。新しい受付とshutdownは同一transactionではないので、運用では先にadmissionを止める必要がある。

## 費用と並列上限

東京Linux Shared `c7i.large` の[公開料金](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonEC2/current/ap-northeast-1/index.csv)（2026-10-01適用）$0.11235/時と[public IPv4](https://aws.amazon.com/vpc/pricing/) $0.005/時から、1台は約$0.11735/時、5台は約$0.58675/時。5台を730時間維持すると約$428.33。EBS・S3・ECR・Lambda・DynamoDB・SQS・logs等は別。cold VMごとの起動待ちと通常10秒以内は両立しにくく、warm維持時間・受付上限・許容待ちを一緒に決める必要がある。

上限5台はpoolの通常の同時容量制約であり、請求総額の上限ではない。admission/rate quota、保存量・保持期間、稼働期限、金額予算による停止条件は未実装。本実験のstackにはsandboxの1時間削除規則があるが、本番の費用制御の代わりにはしない。

## 計測の限界・再現

保存済みのcanonical結果を物理VM単位で照合し、同一VMの実行区間に重なりがないこととdistinct nodeのcleanupを確認した。保存前に停止した試行や競合で負けたduplicateの実行区間は含まれない。heartbeatとASG保護状態は原子的な取得ではなく、VM間の区間比較はguest clockの整合を仮定する。sourceの単一consumer・poll予約・退役fenceのテストはこの計測と別の根拠である。

`infra/experiments/workloads.py` は問題登録APIではなく試験入力。製品の全言語・CLI/Nix環境やCCS全適合、本番公開APIの実証ではない。表示のwireは引き続きJSON snapshot pollingで、SSE/NDJSONの購読契約は未実装。内部VM/queueの操作をクライアントに要求する設計ではない。

policy/host 15件、case tree 1件、graph 3件、terminal 5件、artifact 4件の計28テスト、TypeScript型検査、JSON schema検証、Python compile、差分の空白検査を通した。試験後にCDKの無効なdeprecated `cooldown`引数だけを削除した。StepScalingではCloudFormationへ出力されていなかった引数であり、fresh synthのtemplateが実験時と完全一致することを確認した。この後処理は観測JSONにも記録した。

再現時は `infra/PATTERNS.md` に従い、branch prefix付き専用stackをCDKから作成する。`poolProfile`を選び、`run_scaling.py`または`run_drain.py`へstack名とreport出力先を渡す。同名の既存stackは先にdestroyし、成否を問わず終了後にdestroyする。**試験A/Bの両専用stackを削除し、CloudFormationの不存在を再確認した**。検証時刻は各観測JSONの`teardown`に記録した。共有bootstrapは削除していない。[現行構成図](diagrams/judge-pool.png)は[stack-diagram](https://github.com/caru-ini/stack-diagram)で生成し、以前の1台内並列の図とは分離した。
