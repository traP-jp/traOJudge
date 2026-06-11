# API server conventions

この文書は `server/` 配下の実装判断を固定するための規約です。

## Crate responsibilities

- `traojudge-backend_app`: HTTP 境界。Axum router、extractor、HTTP status、OpenAPI 生成 model との変換、tracing/CORS/shutdown、起動時の依存注入だけを持つ。
- `traojudge-core`: 業務ルール。domain model、usecase、repository trait、外部サービス trait、atomic な操作の契約を持つ。
- `traojudge-infra`: 実 I/O。SQL、migration、repository 実装、外部 API client、transaction の `begin` / `commit` / `rollback` を持つ。
- `traojudge-test`: core/usecase/HTTP の検証に使う fixture、fake、test helper を持つ。

`backend_app` から infra の具象 repository を直接呼び出してよいのは、起動時に依存を組み立てる composition root だけです。handler、extractor、HTTP model 変換から infra 型へ手を伸ばしてはいけません。

## Lessons from the old backend_app

旧実装の handler は usecase 呼び出しに寄っていて方向は悪くありません。ただし、次の点はこの repo では繰り返さないでください。

- 巨大な `DiContainer` に全 usecase、全 repository、dev/prod の runtime 切替、外部 client 初期化を詰めない。起動時の wiring と request 時の state 利用を分ける。
- `backend_app` 内の extractor が session repository を直接呼ばない。cookie/header の取り出しは HTTP 境界、session の検証は core の usecase または port 経由にする。
- `AppMode` や環境変数の読み取りを深い DI 型に散らさない。設定は起動時に読み切り、欠落を明示的な起動失敗にする。
- scheduler、通知、外部 job 登録などの業務イベントを HTTP crate の便利な場所に置かない。core の usecase 契約と infra の実装に分ける。
- 中央の巨大な error mapper を育てない。usecase error は core で型を持ち、HTTP status と response body への変換は backend_app の endpoint/adaptor 境界で行う。

## Request flow

handler は次の形を保ちます。

1. path/query/body/cookie/header を HTTP model から typed value に変換する。
2. `AppState` から usecase handle を取り出す。
3. usecase を 1 回呼ぶ。
4. usecase result を OpenAPI response model と HTTP status に変換する。

handler 内で複数 repository を順番に呼ぶ実装は境界違反です。その並びに業務上の意味があるなら core の usecase に移します。

例: submission 作成は、submission row、source、初回 judge run、`current_judge_id` 更新を 1 つの atomic operation として core が表現します。infra はそれを 1 transaction で実装します。`backend_app` は `create_submission` usecase を呼び、`201 Created` の response に変換するだけです。

## State and dependency injection

`AppState` は request handler が必要な usecase handle を保持する軽い container にします。DB 接続、migration、in-memory fallback、外部 client の接続確認を `AppState::new` や extractor へ隠さないでください。

起動処理の標準形は次です。

1. config を環境変数から読み、必須値がなければ起動失敗にする。
2. infra を初期化し、migration や接続確認を実行する。
3. core usecase を infra 実装で組み立てる。
4. `AppState` に usecase handle を渡す。
5. router を組み立てて serve する。

テスト用の in-memory/fake は明示的な test config か `traojudge-test` から注入します。production 設定の欠落を in-memory で黙って補ってはいけません。

## Types at boundaries

- ID は HTTP 境界で parse し、core へは `Uuid` や domain ID 型として渡す。
- 時刻は core/infra では typed time として扱い、文字列化は HTTP response 変換だけで行う。
- OpenAPI 生成 model は `backend_app` の境界型として扱う。core の domain model に生成 model を混ぜない。
- `current_judge_id` は現在の judge run を指す source of truth として扱う。作成順の都合で必要なら nullable にし、同一 transaction 内で後から更新する。

## Transaction ownership

transaction の意味は core が usecase と trait で表します。transaction の機械的な開始、commit、rollback、SQL 実行は infra が持ちます。

`backend_app` に置いてよいのは「この endpoint がどの usecase を呼ぶか」だけです。「この repository 呼び出しとこの update は同時に成功すべき」という知識は core の責務です。

## Tests

- handler のテストでは HTTP status、response schema、認証 extractor の振る舞いを確認する。
- usecase のテストでは業務ルールと atomic contract を fake port で確認する。
- infra のテストでは SQL、migration、transaction rollback、外部 API client の adapter を確認する。

境界が曖昧になったら、まず handler から見た call chain を書きます。handler が repository や transaction を知っていたら、その実装は置き場所が違います。
