# 提出・進捗・結果を、どのAPIと通信で受け取るか

[実演動画（MP4）](demos/judge-flow.mp4) · [画面](demos/judge-flow-poster.png) · [検証記録](demos/judge-flow-evidence.json)

**今回動いているのは polling。通常の HTTP request/response で、最新状態の JSON を繰り返し取得する。SSE・WebSocket・NDJSON subscribe は未実装。** この文書は呼出側の契約を説明する。保存先・キュー・実行ホストの選定は[内部構成](serverless-judge.md)に分ける。

## 現在の通信とデモ用API

ブラウザーは表示サービスに `fetch` する。ローカル実演は loopback HTTP、表示サービスから受付・状態取得サービスへは HTTPS/JSON。ブラウザーが実行環境や保存先へ直接接続する必要はない。デモ用の入出力は次のとおりで、製品APIとは別の検証用契約である。

| リクエスト | 応答・意味 |
| --- | --- |
| `POST /submit` | 同一Originのブラウザーから本文なしで呼ぶ。固定したC17ソース・3ケースを提出し、`202 application/json` で `submission`（提出番号）、`run`（今回の実行ID）、`request_id`、`contest`、`accepted_at` を返す。202は受付であり判定完了ではない。 |
| `GET /status?id={request_id}` | `200 application/json`、`Cache-Control: no-store`。同じ提出・実行の最新状態を返す。下記の `view` と表示用の経過秒を読む。 |

状態応答の主要部分の例（表示用の補助項目は省略）:

```json
{"submission":1,"run":"example-run-id","request_id":"example-request-id","view":{"status":"judging","completed_cases":2,"total_cases":3,"revision":6}}
```

`status` は `queued / judging / completed / infrastructure_error`。最終結果は `completed` のときの `view.verdict`。同じrunのrevisionで新旧を区別する。最初の保存より前はrevisionを省略するため表示側で0と扱う。接続エラーは判定結果に変換しない。この検証用APIは loopback 専用であり、固定問題・本人認証なし。未知ID等の例外は現状500にまとめている。POSTの再送は別提出となり、製品のIdempotency-Key、外部向け認証/CORS・エラー契約には未接続。

通信は **応答を読み終えてから100ms待ち、次のGETを送る**。100ms周期で必ず更新されるという保証ではなく、実際の間隔には通信時間が加わる。1本のHTTP応答を開いたまま変更を配信する方式ではない。全ての中間状態の受信は保証しない。[実際の観測例](demos/polling-coalescing-evidence.json)でも1/3の次に3/3を取得した。ケース結果の欠落とは異なる。切断中も判定は継続し、同じbridgeが動いている間は再GETで最新状態へ追い付ける。bridge再起動後のID復元はこのデモでは未実装。

## 表示契約と並行実行

`i/n` の `i` は **判定が完了したケースの件数**であり、実行中のケース番号・DAGのノード数ではない。ケースの完了順は任意でよい。0件ならWJ、1件なら1/3、2件なら2/3、3件なら3/3。全件の後に最終集約を完了してACとなる。3/3と最終確定は別の状態である。

実行フローは `build → { run/check 1, run/check 2, run/check 3 } → summarize`。build後は全ケースを実行可能にし、summarizeは全ケースの結果を待つ。検証容量は1ホスト・2slotなので最大2ノード同時。ホスト数1と逐次DAGは同義ではない。前の動画で入れたケース間の逐次依存は解釈の誤りだったため除いた。

今回の動画は並行実行を観測しやすいよう、プログラム内でケースごとに3.4秒／1秒／2.2秒待つ。進捗をタイマーで生成することはしない。検証記録で実際の実行区間の重なりと、全ケース後の集約を確認する。これは通常10秒の性能測定とは分ける。

## 既存の製品API契約案との関係

[既存OpenAPI](../API/front_back.yaml)には、次の契約案が既にある。**このデモをそのハンドラーへ接続した実装はまだない。**

| 操作 | 契約案 |
| --- | --- |
| `POST /submissions` | `202`、`SubmissionAccepted`。`submissionId / judgeRunId / problemVersionId / acceptedAt` を返す。ソース転送と提出確定は分ける。 |
| `GET /submissions/{submissionId}/judge-runs/{judgeRunId}` | `200 application/json`、`JudgeRun`。`state / completedCaseCount / totalCaseCount / revision / result / failure` 等を返す。`Cache-Control: no-store`。 |

詳細は[進捗取得の操作](../API/paths/submissions.yaml)・[JudgeRunスキーマ](../API/schemas/submissions.yaml)を正とする。公開できない総数はnull、結果と基盤障害は別項目。同じrun内のrevisionで新旧を判定し、再判定は別runとして扱う。ケース名・入出力・個別判定や内部ノードIDはこの製品の進捗契約に含めない。取得には既存の本人・同一チーム・運営の認可を適用する。

このOpenAPIは最新状態の取得を定義しているが、通知手段・間隔は未決としている。ページング用cursorは購読再開tokenではなく、snapshotのrevisionもイベント配信の再開tokenではない。
また、デモの数値 `submission` と製品案の識別子 `submissionId` は同じ契約ではない。大会内orderingを外部へ公開するキー・比較規則は製品案に明示されておらず、識別子や通知tokenから順番を推測させないよう定義する必要がある。

## 別サイトがsubscribeする場合に必要な契約（未実装）

外部順位表のような独立した利用側を接続するなら、最新状態の取得に加えてイベント配信を設計する必要がある。候補は長時間のHTTP GETで `application/x-ndjson` を配信し、1行を1通知として読み取る方式。SSEなら `text/event-stream` とSSEのフレーミング・再開規則となり、wire形式が異なる。現時点ではどちらも採用・実装済みとはしない。

決めるべき契約は、初期状態と配信開始位置の整合、公開イベントのスキーマ、対象範囲の認可、順序の範囲、重複の扱い、再開token・保持期間・期限切れ時の復元、heartbeat・切断・遅い購読者の扱い。提出の採番順とイベントの配送順は別に定義する。実装のDBやqueueを利用側の契約にはしない。

[ICPC Contest APIのevent feed](https://ccs-specs.icpc.io/2026-01-next/contest_api#event-feed)は、長時間HTTP応答のNDJSON通知と、対応する場合の `since_token` による再接続を定義する。全ての内部更新が1件ずつ必ず通知されるという規格ではなく、重複・集約・順序の保証にも規則がある。独自JSONをNDJSONにしただけではCCS互換にならない。CCS互換の外部順位表を対象にするなら、その公開オブジェクトと認可・freezeも含めた対応を別途実証する。

## 実演の検証・再実行

[録画スクリプト](../../infra/demo/record.py)は2件の採番、途中の完了件数、件数の単調性、最終ACを検証する。受信した中間表示はJSONに記録し、未観測の更新を画面上で補って全件配信に見せることはしない。
更新した動画は39.08秒。2件ともケースの完了順は2→3→1、同じホスト上のcase 2とcase 3のrun区間が約1.027秒重なった。最大同時run数は2。件数表示はこの完了順序に依存しない。

実行環境を[構成文書](serverless-judge.md)に従って用意し、`--context proofName=ParallelDemo` で専用stackを作る。agent ready後、リポジトリ直下から `python infra/demo/server.py --stack codex-serverless-judge-ParallelDemo`、別シェルで `python infra/demo/record.py` を実行する。毎シェルで `/workspace/tooling/env.sh` をsourceし、必要なら同じ `PLAYWRIGHT_BROWSERS_PATH` を録画にも指定する。成功・失敗にかかわらず `infra/` で `npx aws-cdk destroy --all --context proofName=ParallelDemo --force` を実行する。
