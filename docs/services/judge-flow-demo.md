# 提出から WJ → ケース進捗 → AC までの実演

[デモ動画（MP4）](demos/judge-flow.mp4) · [最終画面](demos/judge-flow-poster.png) · [実 API 応答・検証記録](demos/judge-flow-evidence.json)

東京リージョンの実 AWS スタックに C17 の加算プログラムを2件提出する。採番された **#1、#2** について、ブラウザーで **WJ → 1/3 → 2/3 → 3/3 → AC** を確認する。
動画はライブ画面の等速・無編集録画。進捗は実 API の保存済み状態から作り、タイマーで増やさない。プログラム自体が各ケースで100ms待機する。
この画面は `infra/demo/` の検証用ビューで、製品 UI の実装ではない。
動画は **43.08秒**（1440×1040、H.264）。新規スタックの最初の提出は受付から AC の画面観測まで **14.473秒**、続く提出は **9.661秒**。この2件だけで latency SLO の統計的な保証とはしない。

## 実行されるフロー

```text
提出 → 採番 → build
              → run 1 → check 1  [1/3]
              → run 2 → check 2  [2/3]
              → run 3 → check 3  [3/3]
              → summarize       [AC]
```

`flow: "sequential"` を指定すると、既存の Rust Engine にこの8ノードの DAG を渡す。表示上の build は `compile` ノード。各 check から次の run に依存を張るため、順次処理は画面上の演出ではない。既存の並行フローは既定値として保持する。

| 段階 | 実現方法・表示の根拠 |
| --- | --- |
| 提出・採番 | IAM 認証の Lambda API が source/test を S3 に保存。DynamoDB の counter＋receipt transaction 成功で提出番号と run を確定する。HTTP の到着順・実行完了順を採番順とはしない。 |
| durable 実行 | receipt の DynamoDB Streams → run ごとの SQS FIFO → 制御 Lambda。Rust Engine の閉じた SQLite、進捗、dispatch outbox を同じ S3 checkpoint に条件付き保存してから、実行要求を SQS に送る。 |
| 隔離・共有 | warm EC2 の agent が各ノードに新しい gVisor sandbox を作る。guest は network none・read-only rootfs。agent だけが version＋SHA-256 を固定した入力を取得し、成果物を保存する。build の binary 1版を3 run が参照し、check の verdict 成果物3個を summarize が読む。 |
| WJ・途中進捗 | `completed_cases == 0` なら WJ。**check の結果を checkpoint に反映した件数**から 1/3、2/3、3/3 を作る。run の起動要求や node 数で進捗を増やさない。フローカードの「配送済み」は guest の実起動を保証する時刻ではない。 |
| 結果表示 | summarize は各 verdict 成果物を読み、最終 verdict を出力する独立した実ノード。制御側も耐久保存済みの全 case 結果と集約結果の一致を検証する。最後のノード完了によって Engine が completed となった時点で AC を表示する。 |

ブラウザーは localhost の Python bridge を通じて実 Lambda API を繰り返し読む。AWS 資格情報・接続先 URL は bridge 側に置き、HTML・録画・公開レポートに渡さない。各応答の revision、完了件数、Engine の node state、観測時刻を JSON に残す。

## 動画を検証する仕組み

`infra/demo/record.py` が実ブラウザーを操作し、2件それぞれで WJ、1/3、2/3、3/3、AC の全表示を観測するまで待つ。件数の単調性と採番 #1/#2 を検査し、JavaScript エラーがあれば失敗する。
最後に S3 checkpoint と結果を読み、8ノードの完了、全ノードの `runsc`・network none・read-only、binary の版固定と各 run の参照、summarize 入力3個の checksum と AC を確認する。
JSON の表示観測時刻には API 通信・ポーリングの遅延も含む。全中間状態を必ず表示する保証はポーリング一般にはないため、この録画では実際に全状態を観測したことを別途 assert する。

逐次フローは並列性を使わず、summarize の独立起動も含む。これは先行の「2提出並行・3ケース、warm P95 8.511秒」の測定とは異なるワークロードであり、その数値をこの動画に流用しない。新しい計測値は動画内の経過秒と JSON を参照する。

## 再実演

毎シェルで `/workspace/tooling/env.sh` を source し、`infra/PATTERNS.md` に従う。AWS profile は環境の非機密 manifest から選ぶ。Python に `boto3==1.42.20` と `playwright==1.62.0`、実行環境に Chromium と FFmpeg が必要。

リポジトリ直下から準備・デプロイする。同名 stack が既にあれば先に destroy する。管理環境で Docker 設定が read-only の場合は、proxy・CA・既存認証を保持した書込み可能な一時コピーを `CDK_DOCKER` 経由で使い、終了時に削除する。

```sh
source /workspace/tooling/env.sh
bash infra/scripts/prepare-engine.sh
python infra/scripts/prepare-gvisor.py
python infra/scripts/check-sequential.py
cd infra
npm ci && npm run build
npx aws-cdk bootstrap
npx aws-cdk deploy --all --context proofName=Demo --require-approval never
```

EC2 agent の ready heartbeat を確認してから、リポジトリ直下で次を起動する。

```sh
source /workspace/tooling/env.sh
python infra/demo/server.py --stack codex-serverless-judge-Demo
```

別シェルで録画する。新しい bridge の起動ごとに大会 ID を作るため、最初の2件の採番は #1/#2 となる。

```sh
source /workspace/tooling/env.sh
python infra/demo/record.py
cd infra
npx aws-cdk destroy --all --context proofName=Demo --force
```

Playwright の browser cache を環境で指定する場合は、録画にも同じ `PLAYWRIGHT_BROWSERS_PATH` を渡す。初回は `python -m playwright install ffmpeg` で録画用バイナリも準備する。

成功・失敗にかかわらず専用スタックを撤去する。共有 CDK bootstrap は保持する。構成の全体像と障害復旧の試験は[構成・実証結果](serverless-judge.md)、残る設計判断は[関心事](serverless-judge-concerns.md)を参照。
