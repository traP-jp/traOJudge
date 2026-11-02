# AWS 実行基盤の関心事と判断記録

対象は提出→ordering→durable execution→isolate→artifact共有→進捗→結果。実装したこと、実リソースで確認したこと、本番前の未決事項を分ける。
[構成・実証結果](serverless-judge.md)が現在の選定を示す。通常10秒の目標を受け、先行 Fargate 案から **warm EC2＋offline gVisor** に変更した。

| ID・関心事 | 現在の判断・証拠 | 残る判断／限界 |
| --- | --- | --- |
| C01 ordering | 大会内counter＋receiptのtransaction成功が線形化点。並行6提出・連番・単調timestamp・同一要求再送を実証。デモは数値submissionを返す | HTTPの最初の到着順・実行完了順とは別。製品OpenAPIのsubmissionIdだけでは公開orderingキー・比較規則が未定義。厳密な先着実行は別のscheduler契約 |
| C02 受付の重複・応答喪失 | request_id＋本文 hash、同一内容の再送は同じ提出/run。receipt stream が受付 outbox | transaction 後・HTTP 応答前の強制停止は個別には注入していない。異なる本文の key 再利用を許さない |
| C03 durable execution | Rust Engine の閉じた SQLite・進捗・dispatch outbox を1個の S3 checkpoint に保存後に送信。保存直後の実停止から185.413秒で自動復旧 | SQLite commit だけで分散耐久確定と扱わない。送信直後・ACK checkpoint 直前の強制停止は追加の試験候補 |
| C04 isolate | 各 node に新しい gVisor sandbox、guest 全体の network none、read-only rootfs。UID/NNP/seccomp/親 environ 拒否/答えの非提供/資源制限を実証 | 完全な侵入耐性の証明ではない。Linux/Go/gVisor/image の脆弱性対応と独立した監査が必要 |
| C05 latency SLO | 初期7ノードの短いC17・3ケース・2提出並行・10サンプルでwarm P95/最大8.511秒、中央値8.084秒 | summarize追加後8ノードのSLO測定は別。初期6並行混合負荷ではAC22.695秒。到着率・ケース数・言語・CPU使用量別の負荷条件、warm台数・scale-outが必要。全負荷で10秒とは言わない |
| C06 warm と cold | 大会中は1台 warm、休止時0台、次の受付で起動。0台の受付→ready は87.850秒。default warm、休止後 auto mode は最終受付15分＋仕事なしで0台 | cold boot・障害復旧を通常10秒から分ける。大会前の prewarm、稼働時間の費用、mode 変更の運用を決める |
| C07 容量と費用 | c7i.large 2vCPU/4GiB、2 slot。CPU credit を使わない。EC2＋IPv4は約$0.11735/時、EBS等は別。ASG desired/min/instance 数0を実証 | 検証はmax1台。常時warmの月額、ピークの予約台数、並列性・公平性・上限と予算を決める |
| C08 ordering と並行 writer | 受付の順序は大会単位、FIFO の直列化は run 単位。1 run の停止中も同じ大会の他 run が完了。S3 If-Match も使用 | CAS の競合 writer 自体の故障注入は未実施。run で大きな連続更新があるとその run 内で待つ |
| C09 capacity の直列化 | 共有 Lambda quota のため reserved concurrency を使わず、90秒の DynamoDB lease（function timeout30秒）で遷移を直列化 | owner が落ちると lease 待ちが残る。共有アカウントの quota は変更しない。容量復旧の RTO は通常処理と別 |
| C10 scale to zero | 実際にEC2 0台へ移行し、その後の受付で再起動、再び0台に戻ることを実証 | 保管・ログ・空queue照会等の要求料金は残る。月額ゼロとは言わない |
| C11 host loss | 結果/ACK前に管理下のテストVMをpower offし、ASGの別ホスト＋SQS再配信で同じ提出/runが全3ケースACへ復旧。受付応答後から結果・再送確認252.646秒、再送は同じ受付、最後に0台・DLQ空 | 証拠は pool-judge-host-loss.json。host loss、単なるagent再起動、AZ障害の範囲を混同しない。障害時RTOを通常10秒から分ける |
| C12 配送・副作用 | node message は結果保存→完了通知の後に削除。再配信は保存済み結果から通知できる。visibility180秒、実行中30秒毎に延長 | delivery exactly-once ではない。重複計算の費用は残る。非冪等な外部副作用を持つ一般workflowは別設計 |
| C13 artifact の不変性 | key/version/SHA-256を固定。同じsource keyへの上書き後もAC。別sandbox間の受渡しを実証。終了済みguestのoutput読取はrealpathをjob配下に限定し、親symlink経由のhost読取もローカル試験で拒否 | output総量・保持期間・テストデータの閲覧権限を製品側で定義。path検査中にguestが動かない終了順序を維持する |
| C14 権限と転送 | guestにAWS credential/署名URL/Docker socketを渡さず、ホストがS3/SQSを操作。ホストroleは自stackのデータに限定 | ホストagentはtrusted境界。host compromise、管理者権限、image取得先・deploy主体の管理は別の監査 |
| C15 ケース進捗 | check完了の集合を数え、node数で代用しない。WA/TLE/RE/OLEでも全3ケース継続。並行実行では1/3→3/3というsnapshot受信も実測し、polling-coalescing-evidence.jsonに記録 | ケース結果の耐久保存と全中間通知の購読保証は別。case識別・動的対象・case内途中通知、全件通知か最新状態か、遅延・負荷目標を定義 |
| C16 閲覧と実行の寿命 | 保存済みviewを返し、閲覧中断しても実行継続。再取得で確定結果を返す | 製品の閲覧認可、隠すべきcase/score情報、event-feedやpushの選択は未実装 |
| C17 再判定・遅着 | 同じ提出ID・別run。旧runの再配信で新runを変更しない。終端後の正当なsibling結果もACK、DLQは空 | preview/採用切替は製品側。配送ACK喪失時もengine job stateで判断することをローカル5項目で確認 |
| C18 判定と基盤障害 | CE/WA/TLE/RE/OLEとcontainer起動失敗を分ける。基盤障害後の遅着も正常に処理 | sandbox初期化失敗とprogram終了の専用status protocol、compiler内部障害、OOM/MLEの分類は本番前に厳密化する |
| C19 資源の意味 | CPUとwallを分け、SIGXCPUを観測。address-space、出力、process数、外側memory/CPU/pids制限 | address-spaceとRSSは別。複数processの合計CPU/RSS、言語オーバーヘッド、cgroup OOMの扱いを決める |
| C20 backpressure | 2 slotのlong poll、メッセージ保持・可視性延長・DLQ。pool停止はqueue内/in-flight無しを確認 | DLQ再投入、優先度、公平性、到着率別の最大待ち時間、過負荷時の受付制御を決める |
| C21 boot/health | agentのheartbeatをreadyとし、ASG EC2 healthと分ける。初回launch→ready71.446秒 | image/agent初期化失敗のwatchdogと置換基準は未実装。sandboxのSSM管理roleは未設定なのでconsole/readyで観測し、SSM設定を回避しない |
| C22 supply chain | gVisor release-20261026.0をSHA-512で固定、失敗時runcへfallbackしない。imageはCDK asset hash tagで参照 | image digestの明示固定、tag改変権限、パッチ時のdrain/image cache更新、Nix言語環境との接続 |
| C23 checkpoint互換 | payload revisionとは別にSQLite実schemaを含むcheckpointを使う | schema version/migration/rollback、旧binaryとの互換条件、巨大DAGのcheckpoint転送費用を決める |
| C24 休止操作と受付 | capacityはleaseで直列化、手動休止は仕事なしで実行。reconcileは1分間隔 | SQS近似件数と受付transactionは同一でない。productionの休止にはadmission gate/drain手順が必要 |
| C25 観測と情報公開 | contest/submission/run/nodeで追跡。公開reportに実account ID/ARN/接続先/署名URLを保存しない | queue/worker/進捗遅延のalert、監査保存期間とSLOダッシュボードを決める |
| C26 保持・削除 | sandboxはDESTROY、S3の全versionも削除、root EBSはinstance終了時削除。専用4stackの撤去は pool-judge-cleanup.json で確認、共有bootstrap/asset保管は保持 | 本番はbackup/復旧/保持規則を別に設定。sandboxの作成1時間後削除を本番の保持方針にしない |
| C27 障害注入の権限 | checkpoint後停止とhost poweroffはIAM保護した独立のinfra fixtureだけで明示指定 | 製品の提出APIへ注入flagをそのまま公開しない。guestにはhost管理操作を渡さない |
| C28 CCS適合範囲 | 採番・native自動判定・資源制限・進捗・再判定の実経路を確認 | fixtureはC17/加算問題。UI/認証/大会設定/scoreboard/freeze/Contest APIの完全互換性は別タスク |
| C29 並行DAGと容量 | build後に全run/check枝を実行可能にし、全checkから独立summarizeへjoinする。1ホスト2slotで実際のrun区間の重複を検証。デモに誤って入れた逐次依存とflow選択は除いた | 1ホストでも複数slotで並行可能。n枝と同時実行n件は別。初期7ノードのP95をsummarize追加後8ノードや、長い待機入りの可視化用動画へ流用しない |
| C30 表示API・wire契約 | 現行デモはHTTP request/response＋application/jsonの最新snapshot polling（応答後100ms）。SSE/NDJSON subscribeは未実装。既存OpenAPIのJudgeRun取得契約案とは別のfixture | 公開契約はdomainの提出/実行ID・完了数・revision・結果に限定する。認可、遅延/レート、外部の独立consumerへの契約接続を実証する必要がある |
| C31 購読と再開 | CCSのNDJSON event-feedを参照し、最新状態取得と全通知/再開の保証を分ける。独自NDJSONをCCS互換とは呼ばない | 採用方式、初期snapshotと境界、token/保持/期限切れ、重複/順序/間引き、heartbeat、遅いconsumer、閲覧認可とfreezeを決める。現デモbridgeの再起動後ID復元も未実装 |

## 実装で確認した落とし穴

- Engine callback を直接外部起動にすると、DB/checkpointより先に副作用が起きる。callbackは収集し、checkpointとoutboxを保存後に配送する。
- 結果受信のたびに全pending nodeを送ると重複計算が増える。送信ACKを保存し、停止時の再送だけを許す。
- `RLIMIT_NPROC` は共有kernelではLinux UID全体に効く。開発用UID1000との衝突を避ける必要があり、gVisorではnodeごとのkernel境界・外側pids制限も使う。
- queue送信だけで「実行開始」と表示しない。fixtureは最初のnode結果までqueued。厳密な開始時刻は別途記録する。
- private umaskの0600をDocker COPYするとLambdaの非root UIDやvalidatorが読めない。配布modeを明示し、非rootで実行を確認する。
- 新規S3 bucketのglobal endpointは307 TemporaryRedirectを返す。署名URLをredirectすると署名が無効になる。先行Fargateの403は307のXML codeで切り分け、regional endpointとredirect拒否へ修正した。poolのguestにはURL自体を渡さない。
- 正当な遅着を「pendingにない異常message」として扱うとDLQを汚す。送信ACKを失った場合も、耐久確定したjob stateから終端runの遅着を判断する。
