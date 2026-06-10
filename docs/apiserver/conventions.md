# API server コーディング規約

このディレクトリは traO Judge の API server (`server/`) を対象としたコーディング規約をまとめたものです。
**機械的に検証できる項目は `rustfmt` / `cargo check` / `cargo clippy` に寄せる** 方針です。このドキュメントは「なぜその分割にするのか」「formatter や lint で表現できない設計意図と運用」を残すためのものです。AI エージェントは check 指摘を回避するためだけの書き換えをせず、ここに書かれた意図に沿って書き直してください。

## スタック

- Rust 2024 edition
- Axum (HTTP server / routing)
- Tokio (async runtime)
- tracing / tracing-subscriber (structured logging)
- API schema は `docs/API/front_back.yaml`
- DB schema は `docs/database/*.puml`
- サービス境界は `docs/services/architecture-overview.puml`

## Crate 構成と責務

- `server/traojudge-core/` - ドメイン型、value object、ドメインルール、usecase、repository / port trait
- `server/traojudge-infra/` - DB、Storage、Queue、OAuth、Mail など外部 I/O、実際の DB 更新手順、transaction を持つ repository 実装
- `server/traojudge-backend_app/` - Axum router、handler、request / response DTO、extractor、HTTP error mapping
- `server/traojudge-test/` - fake repository、fixture、integration test helper

依存方向は以下を守ってください。

- `traojudge-core` は他の workspace crate に依存しない
- `traojudge-infra` は `traojudge-core` の trait を実装し、DB 固有の更新順序と transaction を閉じ込める
- `traojudge-backend_app` は `traojudge-core` と `traojudge-infra` を組み立て、HTTP request を core usecase へ渡す
- `traojudge-test` は production code から参照しない

`backend_app` は外から見える API の境界です。handler に業務フローや DB 更新手順を直接書かず、core usecase を呼び出してください。core usecase は業務フローと「atomic に満たすべき操作」の契約を表し、実際の DB 更新順序と transaction は infra の repository 実装に閉じ込めます。

`backend_app` 内の module は以下の分割を基本にします。

- `http` - router、handler、extractor、OpenAPI DTO、HTTP response 変換
- `error` - core / infra error から HTTP error への変換
- `state` - app-wide dependency の組み立て

`core` 内の module は以下の分割を基本にします。

- `domain` - domain model、value object、ドメインルール
- `usecase` - application service、権限判定の呼び出し、port 呼び出しによる業務フロー
- `port` - repository / external service trait と、atomic に満たすべき操作の契約

usecase 専用 crate は当面作りません。将来 `core::usecase` が大きくなった場合に、`traojudge-usecase` へ切り出せるように境界だけを保ちます。

## 自動チェック

- `cargo fmt --all --check` - Rust の整形確認
- `cargo check --workspace --all-targets` - workspace 全体の型チェック
- `cargo clippy --workspace --all-targets -- -D warnings` - warnings を許容しない lint
- `nix run .#ci:treefmt:check` - repository 全体の formatter 確認

エラーが出た場合は、警告を黙らせるための局所的な回避ではなく、型・責務・依存方向を見直してください。

---

## 機械的に寄せたい規約と意図

### Rust

- **公開 API の型は明示する**
  - handler、usecase、repository trait、infra 実装の戻り値は読み手に契約を伝える場所です。推論に任せすぎると、crate 境界で何を保証しているのか分かりにくくなります。
- **`unwrap` / `expect` は境界で使わない**
  - HTTP request、DB row、外部 service response は信用できない入力です。失敗を domain / usecase / infra error に変換し、最終的に `backend_app` で HTTP status に対応させます。
- **`todo!` / `unimplemented!` は production path に残さない**
  - 一時的な stub が必要な場合は、明示的な error variant を返して呼び出し側で扱える形にします。
- **`#[allow(...)]` には理由を書く**
  - 例外が必要な場合は、なぜその lint を外すのかをコメントで残します。理由のない allow は規約への信頼を落とします。

### Async / I/O

- **core に I/O を入れない**
  - core は DB pool、HTTP client、環境変数、現在時刻、乱数を直接読まないでください。必要なものは trait または input として渡します。
- **core usecase は DB 更新手順を持たない**
  - core usecase は「どの操作が atomic であるべきか」を port の契約として表現します。`begin -> update table A -> update table B -> commit` のような DB 固有の順序は infra に置きます。
- **atomic 性が不要な orchestration は core usecase で組み合わせてよい**
  - 読み取り系や、複数 repository 呼び出しが同じ transaction に入る必要がない処理は core usecase で組み合わせます。一方、複数 table 更新を atomic にしたい処理は infra の 1 repository method に寄せます。
- **blocking I/O や重い同期処理を async handler で直接実行しない**
  - ファイル I/O、圧縮、大きな hash 計算などは専用 executor や外部 job へ逃がせる形にします。

### HTTP / Axum

- **handler は薄く保つ**
  - handler の責務は request extraction、DTO validation、usecase 呼び出し、response 変換です。権限判定、集計、DB 更新手順を handler に増やさないでください。
- **handler は transaction を触らない**
  - transaction が必要な API でも、handler は core usecase に input を渡すだけにします。commit / rollback の責任を handler に置くと HTTP 都合と DB 実装都合が混ざります。
- **OpenAPI DTO と domain model を混ぜない**
  - `docs/API/front_back.yaml` の request / response は HTTP 表現です。core の型とは別にし、`From` / `TryFrom` などで変換します。
- **HTTP status への変換は一箇所に寄せる**
  - 同じ domain error が handler ごとに違う status へ変換されると API が不安定になります。error mapping は共通型に集約してください。
- **path parameter を正とする**
  - body に ID が含まれる API では、path と body の不一致を `400 Bad Request` として扱います。特に editorial の `problemId` は database docs の記述に合わせます。

### DB

- **`docs/database/*.puml` を DB 設計の source of truth とする**
  - schema、nullable、unique、soft delete、履歴保持の意図は database docs を優先します。実装都合で変える場合は先に docs を更新してください。
- **soft delete を通常の delete と混同しない**
  - `problems` と `editorials` は `deleted_at` による soft delete です。通常の一覧・詳細では `deleted_at IS NULL` を基本条件にします。
- **token は生値を保存しない**
  - session token、developer token、verification token、OAuth state は hash だけを保存します。生 token を返すのは発行時の response だけです。
- **current judge run だけを submissions の現在値へ反映する**
  - 古い judge run が後から完了しても、`submissions.current_judge_id` と一致しなければ `submissions` の集約値を更新しません。
- **rejudge は履歴を上書きしない**
  - rejudge では新しい `submission_judge_runs` を作り、古い run と result は履歴として残します。
- **solved count は集計結果として扱う**
  - `problems` に solved count を永続化せず、`problem_solved_users` から返します。rejudge で AC 状態が変わる場合はこのテーブルを再計算します。

### Auth / Permission

- **認証と認可を分ける**
  - 認証は「誰か」を特定する処理、認可は「その人が操作できるか」を判定する処理です。Axum extractor で user を取り出した後、操作ごとの権限判定は core usecase から core のドメインルールを呼び出して行います。
- **認証手段とプロフィール項目を混同しない**
  - OAuth 認証に使う ID は `user_auth_identities`、プロフィール表示用の traQ / GitHub ID は `users` に置きます。bind / revoke でプロフィール値を暗黙に変更しません。
- **revoke / logout は削除ではなく失効にする**
  - session と developer token は `revoked_at` を入れて無効化します。監査や一覧表示に必要なメタデータを残します。

### Logging

- **request に紐づく情報は structured field として出す**
  - `tracing::info!("... {id}")` の文字列埋め込みではなく、`tracing::info!(%problem_id, "created problem")` のように field 化します。
- **secret を log に出さない**
  - token、password、authorization code、OAuth state、生の source bundle は出力禁止です。必要な場合は hash や ID に置き換えます。
- **domain error と internal error を分ける**
  - 入力不正や権限不足は通常の API 応答です。DB 接続失敗や外部 service failure は internal error としてログの粒度を上げます。

---

## lint で表現できない設計方針

### API schema との対応

- route、request、response、status code は `docs/API/front_back.yaml` を正にします
- OpenAPI DTO は `server/traojudge-backend_app/scripts/gen-api.sh` で生成します
- 生成済み DTO が最新かは `server/traojudge-backend_app/scripts/gen-api.sh --check` で確認します
- `server/traojudge-backend_app/src/models/` は OpenAPI からの生成物なので手で編集しません
- OpenAPI と実装が食い違う場合、先にどちらが正しいかを決めてから片方を直します
- response DTO の field 名は OpenAPI の camelCase に合わせます
- Rust 内部の domain / DB 型は snake_case の Rust 命名を使い、境界で変換します

### Error 設計

- domain error はドメインルール違反を表す
  - 例: invalid difficulty、invalid judge status transition
- core usecase error は API 操作としての失敗を表す
  - 例: not found、forbidden、invalid input、conflict
- infra error は外部 I/O の失敗を表す
  - 例: DB query failure、storage failure、queue publish failure
- HTTP error は `backend_app` で表現する
  - 例: `404 Not Found`、`403 Forbidden`、`409 Conflict`

同じ失敗を複数の error type で表現しないでください。DB 固有の失敗は infra で core の port error へ変換し、core usecase が必要に応じて usecase error へ変換し、最後に `backend_app` で HTTP error へ変換します。

### Repository trait

- trait は core に置き、infra が実装します
- trait method は core usecase が必要とする単位で切ります
- DB table と 1:1 の CRUD trait を先に作らないでください
- transaction が必要な操作は、複数 method の呼び出しを handler や core usecase で組み合わせるのではなく、atomic な 1 repository method として表現します

例: submission 作成では `submissions`、`submission_sources`、最初の `submission_judge_runs`、`current_judge_id` 更新が同じ transaction に入るため、個別 CRUD を handler で順番に呼ばないでください。

### Transaction orchestration

- `begin` / `commit` / `rollback` の権限は infra が持つ
- handler と core usecase は transaction gateway を直接操作しない
- core usecase は atomic に満たすべき操作を port method の契約として表す
- infra は DB 具体型を隠し、必要な複数 table 更新を 1 method 内の transaction で実行する
- infra method が途中で error を返す場合は rollback される形にする
- commit 後に外部 queue publish が必要な場合は、port の契約で DB 更新と外部副作用の順序を明示する

core usecase は orchestration を一切しないわけではありません。atomic 性が不要な読み取りや検証のための複数 repository 呼び出しは core usecase で組み合わせてよいです。複数 table 更新を atomic にしたい場合だけ、infra の 1 repository method に寄せます。

### ID / Time / Hash

- ID 生成、現在時刻、token 生成、hash は直接呼び出しを散らさない
- test で差し替えたいものは trait または small provider に分離する
- timestamp は DB と application のどちらが正かを core usecase ごとに決める
- password / token hash の algorithm は型や関数名で用途を分ける

### Test

- core のドメインルールは純粋関数または小さい domain object の単位でテストします
- core usecase は fake repository でテストします
- infra は migration / query / transaction の integration test を重視します
- `backend_app::http` は router に対する request / response のテストを置きます
- 認証、権限、soft delete、rejudge、token 失効は regression test を優先して追加します

### コメント

- WHAT (何をしているか) ではなく WHY (なぜそうしているか) を書きます
- DB docs や OpenAPI と対応する複雑な制約は、該当 docs の path をコメントに含めてもよいです
- 一時的な制限や未実装は、issue 化できる粒度で理由と解除条件を残します
