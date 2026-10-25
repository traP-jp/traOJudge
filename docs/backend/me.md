# 本人情報とアカウント管理

`/api/v1/me` 配下はセッションCookieから本人を特定する。対象の利用者IDを要求本文や独自ヘッダーから受け取らず、サービス管理者も他の利用者の情報を操作できない。

## 実装したAPI

| API | 動作 |
| --- | --- |
| `GET /me`・`PATCH /me` | 本人のプロフィール・役割の取得と、版を指定したプロフィール更新 |
| `GET /me/roles` | 本人の役割と対象範囲の取得 |
| `GET /me/sign-in-methods` | 本人のWebログイン方法のメタデータ取得 |
| `DELETE /me/sign-in-methods/{id}` | 本人の外部ログイン方法を削除。メールは対象外。最後の方法なら409 |
| `POST /me/sign-in-methods/external-authorizations` | 外部アカウント追加の認可操作を開始 |
| `POST /me/sign-in-methods/external-authorizations/{id}/complete` | 検証した外部identityを本人に追加 |
| `GET /me/developer-tokens`・`POST /me/developer-tokens` | 本人の開発者トークン一覧と発行。秘密は発行時だけ返す |
| `DELETE /me/developer-tokens/{id}` | 本人のトークンを失効。既に失効済みなら204 |
| `POST /me/email-changes` | 新しいメールアドレスの確認を依頼 |
| `POST /me/email-changes/complete` | 確認トークンを消費してメールを変更し、他のセッションを失効 |

メール配送・外部OAuthの本番接続はまだない。Coreの `AccountExternalServices` が配送・認可URL作成・認可コード照会の契約を定義し、結合テストではテスト用アダプターを注入する。通常起動では `UnconfiguredAccountServices` を使い、配送依頼と認可開始を503にする。登録・ログイン時のセッション発行、新規登録、レート制限はこの実装範囲に含めない。

## 責務とUnit of Work

- `bootstrap` がRepository・外部接続・ユースケースを組み立て、`http::build_router` に渡す。
- HTTPルートで `nest("/api/v1", ...)`、その配下で `users`・`me`、さらにログイン方法・開発者トークン・メール変更を組み立てる。各機能は相対パスだけを定義する。
- セッションの照合とCookie更新、変更要求のOrigin検証は個別のmiddlewareで行う。`/me` のキャッシュ禁止もルーターへ適用し、パス文字列による分岐を使わない。API外の未定義パスにはAPI用middlewareを掛けない。
- handlerは `CurrentSession`・`ResourceId`・`IfMatch`・`StrictJson<T>` で入力を受け、ユースケースを呼ぶ。Repositoryへ直接アクセスしない。生成DTOとの変換は `http/mapping.rs`、エラー応答は `http/error.rs` に集約する。
- Coreの `AccountService` が所有者・版・最後のログイン方法などの業務判断と確定の順序を担う。役割・ログイン方法はCoreのenumで表し、文字列をJSONへ再変換して判別しない。

`AccountUnitOfWorkProvider::begin(actor)` は本人の行をロックし、セッションを再検査して、本人に限定した永続化操作を返す。Coreの `AccountUnitOfWork` 契約をInfraの `PgAccountUnitOfWork` がSQLxのトランザクションで実装する。ユースケースが必要な操作を組み合わせ、成功時に `commit(self)` する。`rollback(self)` は明示的に破棄を待ち、未確定のUoWは破棄時にロールバックする。確定・破棄は所有権を消費するので再利用できない。commit失敗時に確定結果が不明になる場合は、成功応答を返さず、同じUoWを再試行しない。

セッション照合は独立した `SessionService` と `SessionRepository` を使う。メール配送・OAuth照会中はUoWを保持せず、外部照会の後は新しいUoWで期限・セッション・競合を再検査してから確定する。

## 認証・更新・並行処理

`web_sessions` はセッションUUID・本人・秘密のSHA-256ダイジェスト・期限・失効時刻を保存する。秘密は244bitのランダム性を持つ64桁の文字列とし、生の秘密をDBやログへ保存しない。middlewareは未失効・期限内のCookieだけを受理して期限を90日後まで延長し、公開プロフィールの取得にも適用する。Cookieは `HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=7776000` のhost-onlyとする。将来のログイン処理も同じ属性を使う。

`/me` の応答はエラーも含め `Cache-Control: no-store`。セッションがなければ401、期限切れ・失効済みを復活させない。重複Cookieも認証しない。開発者トークンをWebセッションの代わりには使えない。変更操作は `TRAOJUDGE_WEB_ORIGIN` とscheme・host・portが一致するOriginだけを受理する。未指定・`null`・不一致は403。

各アカウント操作は利用者の行をロックし、有効な本人のセッションを再検査する。プロフィールの版確認と更新、最後のログイン方法の保護、メール切替・確認トークンの消費・セッション失効は、それぞれ同一トランザクションで行う。

ETagは `revision` のbase64url表現に引用符を付ける。取得した値をそのまま `If-Match` に送る。未指定は428、形式不正は400、未知または古い版は412。役割も `/me` の表現に含まれるため、`account_roles` 変更時はDBトリガーで本人の `revision` も更新する。プロフィールと役割の取得にも同じ利用者ロックを使う。

省略したプロフィール項目は保持し、外部ユーザー名への空文字は削除とする。空の更新・未知の項目・`null`・文字列以外・文字数超過は422。Unicodeの文字数で検査する。NULはPostgreSQLのTEXTに保存できないため422とする。

## メール・外部認証・開発者トークン

メールは `sign_in_methods` のemail行に保存する。他人のメールへの変更依頼も202だが、トークンも配送も作らず、既存の未使用トークンを失効させない。新しい確認トークンを発行した場合だけ以前のものを失効する。確認は30分間・本人限定・一度だけ。成功時だけ現在以外のセッションを失効し、宛先競合なら409で全変更をロールバックする。

配送が未設定なら、メールの登録状況を調べる前に一律503。設定済みアダプターの配送失敗は登録の有無を応答へ漏らさないよう同じ202とし、[メール配送の方針](../services/aws-building-blocks.md)に従い自動再試行せず利用者の再依頼で対応する。

外部認可操作は本人・開始セッション・provider・state・PKCEに紐付き、10分で失効する。stateはダイジェスト、PKCE verifierは照会に必要な短命の値として保存し、成功時に消去する。認可コードを外部アダプターで照会し、providerが検証した安定したsubjectだけを保存する。メールの一致でidentityを統合しない。1人につき同じproviderは1つ、同じidentityは全体で1人まで。完了済みの再送は200で同じ方法を返し、削除した方法を同じ操作の再送で再追加しない。

開発者トークンもDBにはダイジェストだけを保存する。有効期限の省略は無期限、指定時は未来の日時だけを受理する。`SessionRepository` の `developer_token_user` は失効・期限を検査するCLI認証用の契約で、CLIの操作権限は各ユースケース側で検査する。発行・失効によって役割やWebログイン方法は変わらない。

## 起動と検証

```sh
TRAOJUDGE_DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge \
TRAOJUDGE_WEB_ORIGIN=https://judge.example \
  cargo run -p traojudge-backend_app --bin backend_app
```

`TRAOJUDGE_WEB_ORIGIN` はscheme・host・portのみ。パス・クエリ・フラグメント・認証情報は含めない。既存の利用者にローカル検証用セッションを作る場合は、実データと分けたDBで実行する。次の固定の秘密はローカル動作確認専用。

```sql
INSERT INTO web_sessions (user_id, token_digest, expires_at)
VALUES ('a8098c1a-f86e-4f70-917b-72f7c091480c',
        sha256(convert_to('local-session-for-testing-only-00000000000000000000000000000000', 'UTF8')),
        CURRENT_TIMESTAMP + INTERVAL '1 day');
```

```sh
curl -i --cookie 'traojudge_session=local-session-for-testing-only-00000000000000000000000000000000' \
  http://127.0.0.1:3000/api/v1/me

DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge \
  cargo test -p traojudge-test --test me -- --include-ignored
```

SQLxが作成・削除する専用DBで、UoWのcommit・明示rollback・未確定破棄、各管理ルートの認証、Cookie・Origin・プロフィールの競合・文字数・秘密の非公開・認証手段の所有者/最後の方法・メール変更の競合/期限/再送/消費・セッション失効・外部identity検証とブラウザの対応を確認する。
