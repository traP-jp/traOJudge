# 現行の関心事・受入条件

現行の前提は **1 VM・同時1実行、最大5 VM**。policyの正本は [pool.json](../../infra/policy/pool.json)、方針は[policy文書](judge-pool-policy.md)、実測の判定は[実験レポート](scaling-experiment.md)。過去のC01〜C31の説明は[当時のcommit](https://github.com/traP-jp/traOJudge/blob/c66c0c6aebaadb9bbecf2360a3493f0de070bd8b/docs/services/serverless-judge-concerns.md)にのみ残す。以前の未検証事項を達成済みとして扱わない。

| ID | 現在の論点・受入条件 |
| --- | --- |
| C01 ordering | receiptの採番と、実行順・通知順は別。製品APIで公開する比較可能なorderingキーは未定義。 |
| C02 受付・重複 | 実験APIは本文hashとrequest_idで再送を識別。製品の提出確定契約との接続は別。 |
| C03 耐久確定 | 閉じたEngine DB・view・outboxを保存後に配送し、再送で未配送を修復。ローカルDB commitだけを耐久確定としない。 |
| C04 隔離・信頼性 | 同時に別のuntrusted実行を同じVMへ配置しない。各実行はoffline sandbox。物理VMの再利用は検証済みcleanup後のみ。 |
| C05 latency | 過去の2slot/P95を流用しない。VM起動・alarm周期・木のgate時間を含む実測と、通常10秒の目標を区別する。 |
| C06 warm/cold | warm容量の費用と、再起動・退役中の提出の待ち時間を測る。回収済みVMをそのまま再利用できるとは言わない。 |
| C07 費用・上限 | ASGとpolicyで通常の目標上限5。単一AZ・0/100置換を宣言。最大台数は請求総額上限ではない。入口・保存・観測・保持・稼働時間の予算も必要。 |
| C08 writer | run単位FIFO＋conditional checkpoint。複数runの並行と、同一runの更新直列化を分ける。 |
| C09 policy | 台数・horizon・retry・保護・配置条件は独立したversioned policy。実行中に任意の台数・方式へ書き換えるAPIを持たせない。 |
| C10 scale to zero | drainをACKし、ASGが0台になった事実を確認する。unknown/boot-stuck/quarantinedが0台を妨げる条件も失敗として扱う。 |
| C11 host loss | 通常scale-inと、EC2障害・強制停止を分ける。強制喪失時は再配信による復旧を測る。停止中の処理が絶対に回収されないとは言わない。 |
| C12 retry | 結果保存→通知→元要求ACK。再配信は保存済み結果を使える。exactly-once実行ではなく、上限付きのat-least-once試行。 |
| C13 artifact/cleanup | 入力version/hashを固定。cleanupはcontainer・作業領域・distinct nodeを照合。回収でcleanup通知を失った場合はVM破棄の確認と分ける。 |
| C14 権限 | guestにAWS資格情報を渡さない。容量計算roleにSetDesiredCapacityを与えない。hostの保護操作は自stackに限定する。 |
| C15 木・fail-fast | 現行は非ACをcheck jobの失敗へ変換し、既存Engine.job_failedに渡す。Cの動画で、発行済みrunの終了と、全ての新規dispatchの停止を区別して確認。Aの独自な子孫SKIPは削除し、過去の測定条件としてのみ残す。 |
| C16 表示 | 現状の表示はsnapshot polling。切断後の最新取得とイベントの再送保証は別。 |
| C17 再判定 | 回収再試行は同じrun、基盤障害で終端化した後の明示再判定は別run。古い結果を混ぜない。 |
| C18 DLQ | NodeDLQを隔離保存後、基盤障害を終端化する。無限retryしない。ControlDLQとreceipt stream枯渇の回復は未完成。 |
| C19 資源計測 | CPU/wall/address-space/RSS、失敗した試行、clock skewを区別。保存済み成功区間だけで全試行の物理的同時性を証明したとしない。 |
| C20 backpressure | 5台を超える需要はqueueで待つ。任意の到着率に10秒を保証しない。admission/fairness/最大待ち時間を別途決める。 |
| C21 health | boot-stuck/heartbeat欠落/cleanup quarantineはfail-closed。自動交換がないと仕事と課金容量が滞留する。本番受入のliveness・費用上のblocker。 |
| C22 供給物 | gVisorの版/hash固定、image取得と更新・drain・VM破棄の方針を定義する。 |
| C23 互換性 | Engine schema、checkpoint、問題定義・policyのversionを区別し、移行・rollbackを検証する。 |
| C24 drain | token付きの不可逆な退役を保存し、予約済みpoll・実行・配送・cleanup完了後だけ保護を解除。active=0だけではACKにならない。古いscale-in通知を受け得るVMをresumeしない。新規提出の追従遅延も測る。 |
| C25 観測 | heartbeatは原子的なASG状態取得ではない。物理VM IDを公開せずopaque labelで照合。サンプリングの限界をレポートに残す。 |
| C26 teardown | 専用stackとversion付きデータを撤去。共有bootstrapは残す。本番の保持規則とは分ける。 |
| C27 障害注入 | 実験だけの明示flagで行う。製品提出APIへhost停止等の権限を公開しない。 |
| C28 接続範囲 | C17/固定fixtureから実Engineと実AWSを検証。製品の言語環境・問題登録・CCS全適合の実証とはしない。 |
| C29 先読み | wall制限を仮の所要時間とした全gate通過シナリオ。真の将来需要の保証でも、horizon内需要の厳密上界でもない。新しい提出やgateの成否は予知できない。 |
| C30 wire契約 | 製品OpenAPIの状態取得案と、実験APIのJSONを分ける。内部のqueue/VM/DBを利用側へ要求しない。 |
| C31 subscribe | SSE/NDJSON・token・保持・freeze・公開認可は未実装。任意の別サイトが購読できるとはしない。 |
| C32 metric feedback | 過去の高いMaximumと退役/cooldown中の現在容量再発行が余剰増減を長引かせた。raw需要・safe target・alarmの遅延を別々に観測し、queue近似値のせいだと断定しない。 |
| C33 poll予約 | active=0でもlong receiveは要求を受け得る。poll→実行の予約引継ぎとdrainを同じlockでfenceし、受信待ちを残して保護を解除しない。実環境でreserved_before_drainからの実行を確認。 |
| C34 時刻・実験版 | 受付、実行終了、durable checkpoint公開、runnerの終端観測を区別する。修正前後のAWS試験とsource/policyのfingerprintを分け、後の修正で古い未達判定を塗り替えない。 |
| C35 撤去順序 | 保護付きVMを残して直接destroyするとASG縮小が進まず、VPC接続の削除もタイムアウトした。controllerが生きているうちにdrain・希望0・所属0を確認してからdestroyする。未実行の先行stackでは空queue等を確認した撤去限定の保護解除とCFN再試行が必要だった。 |
| C36 取消後の観測 | Engineはcancelled後の遅い完了通知を無視する。job状態と、発行済み外部実行の終了・cleanupを分けて観測する。Cの観測adapterは結果を読むだけでscheduler状態を完了に書き換えない。製品の状態取得契約への反映は別途必要。 |
