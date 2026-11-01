# 通常10秒を目標にした AWS native ジャッジ基盤

`codex-serverless-judge` では **warm な EC2 pool＋ノードごとの新しい offline gVisor sandbox** を選んだ。
短い C17 の提出・3ケース・2提出並行の10サンプルで、受付から結果取得まで中央値 **8.084秒**、P95・最大 **8.511秒** を実測した。
休止時の EC2 0台、次の受付による再起動も確認した。cold start と障害復旧は10秒の測定から分ける。

判断の根拠・未決事項は[関心事の記録](serverless-judge-concerns.md)、構成図は[HTML](diagrams/traojudge-architecture.html)／[PNG](diagrams/traojudge-architecture.png)／[dark PNG](diagrams/traojudge-architecture-dark.png)。

## 選定

| 領域 | 構成・理由 |
| --- | --- |
| HTTPS 受付・参照 | IAM 認証した Lambda Function URL。製品 UI/API を作り足さず、実リソースを通して検証する |
| ordering | DynamoDB on-demand。大会内 counter と request receipt を条件付き transaction で同時に確定 |
| 受付 outbox | receipt INSERT の DynamoDB Streams → relay。受付後の API プロセスの寿命と配送を分離 |
| workflow | 既存 `traojudge-workflow::Engine` を Lambda から呼ぶ。SQS FIFO の group は run ごと |
| durable state | 閉じた SQLite 作業コピー・表示用進捗・dispatch outbox を1個の S3 checkpoint に conditional PUT |
| ready node の配送 | SQS Standard。ホストが long poll し、実行中は visibility を延長 |
| native 実行 | EC2 `c7i.large` 1台、2 slot。CPU credit を使う instance を避け、warm な image/cache を再利用 |
| isolate | 各 node に新しい gVisor sandbox。ゲスト全体 `--network=none`、read-only rootfs、専用作業領域。ホストの AWS 資格情報を渡さない |
| guest 内の制限 | UID 65532、環境変数の作り直し、no_new_privs、seccomp、CPU/wall/address-space/file-size/process の rlimits。外側にも memory/CPU/pids の制限 |
| artifact | S3 version＋SHA-256 を固定。ホストだけが入力の取得・成果物と結果の保存を行う |
| 容量変更 | ASG min=0/max=1。capacity Lambda＋DynamoDB lease。大会中 warm、休止時0台、受付で再起動 |

ALB、NAT Gateway、常駐 RDS、EKS は作らない。既存 PostgreSQL の製品 API を DynamoDB に移す変更ではない。

先行の Batch on Fargate 案は正しさの試験を通したが、3ケースの通常判定に約2分かかった。各 node の SubmitJob→RUNNING は中央値30.9秒、p95 49.454秒だったため、10秒目標の構成として採用しない。
その比較結果は[先行 E2E](serverless-judge-e2e.json)・[先行の遅着試験](serverless-judge-late-result.json)に残した。

## 受付から結果まで

1. API は source とテストを S3 に保存し、key/version/checksum を固定する。DynamoDB の counter＋receipt transaction の成功が受付・採番の線形化点。HTTP 接続が最初に到着した順とは定義しない。
2. receipt の stream event から pool を wake し、run 単位 FIFO に制御要求を送る。異なる run は並行して進める。採番の全順序を、実行開始順・完了順へ強制しない。
3. 制御 Lambda が checkpoint を復元して Rust Engine を呼ぶ。callback は外部起動せず、起動待ちを収集する。SQLite の全接続を閉じ、DB・進捗・outbox を一緒に S3 へ保存してから node queue に送る。配送 ACK は別の conditional checkpoint で記録する。
4. ホスト agent が1 node の要求を受け取り、必要な入力だけを version 指定で取得・検証する。新しい gVisor sandbox に task 定義とローカルファイルだけを渡す。S3 URL、AWS credential、Docker socket、他 node の作業領域を渡さない。
5. guest の supervisor がさらに権限を落として native コマンドを実行する。終了後、ホストは Docker の HostConfig で `runsc`・network none・read-only を確認し、宣言した通常ファイルだけを成果物として保存する。sandbox と作業領域は毎回削除する。
6. ホストは結果 object を conditional PUT し、control FIFO へ完了を通知してから元の node message を削除する。保存後・通知前にホストが失われても、再配信時に保存済み結果を通知できる。重複試行の成果物は別 prefix、最初に確定した結果だけを採用する。
7. Engine が compile→各 case の run→check を依存順に進める。check の完了した集合から進捗を作り、WA/TLE/RE/OLE でも残りを継続する。全ケース終了後に最終結果を確定する。CE と基盤障害は分ける。
8. read API は保存済み view を返す。閲覧の中断で実行は止まらない。再判定は同じ提出 ID・別 run とし、旧結果を混ぜない。基盤障害で終えた run の正当な遅着も、終端状態を変えず ACK する。

ローカル SQLite commit だけを分散永続化とは扱わない。外側の FIFO と S3 If-Match が writer を制約する。配送の exactly-once は要求しない。
遅着を ACK する判断には、送信 ACK の有無ではなく、耐久確定済みの engine job state を用いる。

## 実証結果と範囲

2026-11-02 JST、東京リージョンの実リソースを使用した。

- [Pool E2E](pool-judge-e2e.json)：27項目。並行採番・再送・source 上書き後の版固定・native 実行・隔離・資源制限・進捗・再判定・checkpoint 後の停止からの自動復旧・warm latency・0台への停止。
- [Cold wake と遅着](pool-judge-late-result.json)：8項目。0台で新しい提出を受け付けて再起動し、基盤障害後の sibling の結果を ACK すること、終端を変えないこと、DLQ が空であること、再び0台へ戻ること。
- [実ホスト消失](pool-judge-host-loss.json)：7項目。結果を保存・ACK する前にテスト用 EC2 を power off し、ASG の別ホストと SQS 再配信で同じ提出・同じ run を再開。全3ケース AC、再送は同じ受付、最後に0台、DLQ は空。受付応答後から、復旧結果と同一要求の再送を確認するまで252.646秒。
- [撤去確認](pool-judge-cleanup.json)：比較用を含む専用4スタックが存在しないことを CloudFormation API で確認した。共有 bootstrap と asset 保管は保持。

通常の測定は短い C17・3ケース、2提出並行、10サンプル。中央値8.084秒、P95/最大8.511秒だった。
**これは全言語・全ケース数・任意の負荷で10秒を保証する試験ではない。** 6提出を同時に流す混合障害試験では AC の結果確定まで22.695秒かかり、1台2 slot のこの負荷は10秒を満たさなかった。大会規模・到着率に応じた warm 台数、scale-out、admission/fairness が次の容量判断となる。

初回の EC2 launch→ready は71.446秒。0台からの受付→ready は87.850秒だった。大会開始前に warm を完了させる。
checkpoint 保存直後の強制停止は AWS logs でも確認し、180秒の visibility interval を含め185.413秒で復旧した。その間に同じ大会の他の run は完了した。

CPU 制限は wall timeout と独立した SIGXCPU、出力制限は OLE を実際に観測した。メモリーは address-space 制限であり、RSS と同義ではない。正確な MLE 判定と複数プロセスの合計 CPU/RSS は別の契約が必要。
制御コードのローカル5項目は、送信 ACK を失った後の終端への遅着、未知 node／未配送 node の拒否を確認する。
成果物のローカル4項目は、正常ファイルの読取と、最終 symlink・親 symlink による job 外の読取・通常ファイル以外・上限超過の拒否を確認する。
この realpath 検査の補強は AWS E2E 後の追加であり、この追加差分はローカル試験で検証した。
Rust Engine はプロセス再起動・checkpoint 移動・diamond join・重複完了を確認し、対象クレートの tests/Clippy/format と cargo-hakari、CDK の型チェックを通した。

CCS の[採番](https://ccs-specs.icpc.io/2026-01-next/ccs.html#timestamps-and-ids)・[自動判定](https://ccs-specs.icpc.io/2026-01-next/ccs.html#automated-judgements)・[再判定](https://ccs-specs.icpc.io/2026-01-next/wf_requirements.html#rejudging)を参照する。
CCS 全体の適合試験ではない。fixture は native C17 と加算問題。製品の問題登録・認証・UI・scoreboard/freeze・Contest API event-feed、既存 CLI/Nix の言語環境との接続は対象外。

## Warm と0台の運用

作成時は1台を warm にする。capacity の default mode は warm。休止操作はキュー内／in-flight の仕事がないことを確認して desired=0 と auto mode を保存する。
次の receipt が desired=1 に戻し、auto mode では最後の受付から15分経過し、control/node queue に仕事がなければ0台へ戻す。reconcile は1分間隔。
手動休止は新しい受付を止めた状態で行う。SQS の近似件数は admission の transaction と同じものではない。

共有 sandbox では reserved Lambda concurrency を確保できなかったため、capacity transition は DynamoDB の90秒 lease で直列化する。Lambda timeout は30秒。共有アカウントの quota を変更しない。
ASG の EC2 health と agent/image の ready は別の事実であり、試験は heartbeat を待ってから warm 性能を測る。

## 費用

AWS 公開料金表の東京 Linux/shared `c7i.large` は **$0.11235/時**（確認日2026-11-02、EffectiveDate 2026-10-01）。
public IPv4 の **$0.005/address-hour** を加えると1台の warm 計算＋IPv4 は約 **$0.11735/時**、5時間で **$0.58675**。12 GiB EBS、S3/ECR、Lambda/DynamoDB/SQS、ログ、転送は別。無料枠・割引は前提にしない。
休止で EC2 とその root EBS を削除する。0台でも S3/ECR の保管や空 queue の照会等の要求料金は残るので、月額ゼロとはしない。

[EC2 東京 price list](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonEC2/current/ap-northeast-1/index.csv)、[public IPv4 pricing](https://aws.amazon.com/vpc/pricing/)、[先行 Fargate price list](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonECS/current/ap-northeast-1/index.json)。
先行 Fargate は0.5 vCPU/1 GiBで$0.03081/task-hour、最低60秒。3ケース7 node の compute 下限は$0.0035945/提出だったが、起動待ちが10秒目標に合わなかった。

## 再実行

`AGENTS.md`、対象領域の文書、`infra/PATTERNS.md` を読む。毎シェルで `/workspace/tooling/env.sh` を source する。
この管理環境では `develop-hayatroid` から `codex-<topic>` を作る。PR の指示がある場合の base は `develop-hayatroid`。
AWS CLI/SDK は非機密 manifest の `AWS` profile を選び、認証値を出力しない。

```sh
source /workspace/tooling/env.sh
bash infra/scripts/prepare-engine.sh
python infra/scripts/prepare-gvisor.py
python infra/scripts/check-engine.py target/debug/traojudge-infra-probe
cargo test -p traojudge-workflow -p traojudge-infra-probe
cd infra
npm ci
npm run build
npx aws-cdk bootstrap
npx aws-cdk deploy --all --require-approval never
cd ..
# boto3==1.42.20 を利用できる Python 環境を使う。
python infra/scripts/e2e.py --stack codex-<topic>-Pool --report docs/services/pool-judge-e2e.json
python infra/scripts/check-late-result.py --stack codex-<topic>-Pool --report docs/services/pool-judge-late-result.json
python infra/scripts/check-terminal.py
python infra/scripts/check-artifacts.py
cd infra
npx aws-cdk destroy --all --force
```

gVisor は公式 `release-20261026.0` の distribution を SHA-512 で固定する。archive は生成物で commit しない。
`runtime/engine` も生成物。runtime image は Debian trixie なので、別 build host では glibc 互換性を確認する。
Docker build は既存の proxy/CA を保持し、CA は BuildKit secret mount のみで使用する。image にコピーしない。配布 script は0644、binary は0755を Dockerfile で明示する。

実ホスト消失の試験は専用 stack で行う。`inject_host_loss` は IAM で保護したインフラ fixture の障害注入であり、製品の提出 API にそのまま公開しない。

```sh
cd infra
npx aws-cdk deploy --all --context proofName=HostLoss --require-approval never
cd ..
python infra/scripts/check-host-loss.py --stack codex-<topic>-HostLoss --report docs/services/pool-judge-host-loss.json
cd infra
npx aws-cdk destroy --all --context proofName=HostLoss --force
```

比較用 Fargate は `--context execution=fargate --context proofName=Judge` を指定する。
検証前から同名 stack があれば destroy して作り直す。sandbox の stack は作成1時間後に削除される。終了時は成否にかかわらず専用 stack を削除する。共有 bootstrap stack は他タスクも使用する。
公開レポートに実 account ID、ARN、接続先 URL、署名 URL を含めない。

## 構成図の生成

[caru-ini/stack-diagram](https://github.com/caru-ini/stack-diagram) の `75dbd3e3c6fe28844322e6064d3a947b870a6c61` を使用した。
HTML が出典で、PNG は Chromium で HTML を撮影する。AWS 公式アイコンと Lucide を使い、icon package／単独 icon は commit しない。

```sh
python infra/scripts/diagram.py <stack-diagram>/skills/stack-diagram work/judge-diagram.html
python <stack-diagram>/skills/stack-diagram/scripts/build.py work/judge-diagram.html work/stack-icons \
  docs/services/diagrams/traojudge-architecture.html --fonts <stack-diagram>/skills/stack-diagram/assets/fonts
python <stack-diagram>/skills/stack-diagram/scripts/render_png.py \
  docs/services/diagrams/traojudge-architecture.html docs/services/diagrams/traojudge-architecture.png
```
