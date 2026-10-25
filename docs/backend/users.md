# 利用者の公開プロフィール

`GET /api/v1/users/{userId}` は認証不要で、`id`・`displayName`・`bio`・`githubId`・`atcoderId`・`xId`・`revision` を返す。外部サービスのユーザー名は本人申告で、未設定は空文字。認証情報・メールアドレス・役割は公開プロフィールに含めない。

存在しない利用者とResourceIdの形式に合わないIDは404、DB障害は500とする。エラー応答は共通Errorスキーマに従い、500の詳細は応答へ含めず、`requestId` とともにサーバーログへ記録する。

## 構成

- Coreの `domain/model/user.rs` が `UserId` と `PublicUserProfile` を定義する。
- `application/users.rs` が `UserProfileRepository` を通じて公開プロフィールを取得する。
- Infraの `postgres/users.rs` がPostgreSQLから公開項目だけを読み出す。
- HTTPは生成済みOpenAPIモデルへ変換する。生成型をCore・Infraへ渡さない。

`users` テーブルは公開プロフィールを保存する。IDはAPIの不透明な識別子に対応するTEXTで、UUIDへの変換は要求しない。`revision` も不透明な文字列で、作成・更新する側が値を指定する。表示名・自己紹介の長さや必須項目はDB制約でも保護する。

この段階では利用者登録・認証・本人プロフィール更新・問題検索・レート制限は実装していない。登録のHTTPエンドポイントや認証情報を仮に追加することはせず、動作確認には以下のSQLでプロフィールを作成する。

## ローカル起動と確認

[PostgreSQLの起動手順](../../deploy/database/README.md)に従ってDBを用意し、リポジトリのルートで実行する。

```sh
TRAOJUDGE_DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge \
  cargo run -p traojudge-backend_app --bin backend_app
```

`TRAOJUDGE_DATABASE_URL` は必須。`TRAOJUDGE_ADDR` は省略時 `127.0.0.1:3000`。起動時に接続プールを作成し、埋め込んだマイグレーションを適用してからHTTPの受付を始める。DB接続・マイグレーションに失敗した場合は起動しない。DBユーザーにはマイグレーションに必要なDDL権限が必要。

マイグレーションは `server/traojudge-infra/src/postgres/migrations/` に置く。SQLxが適用履歴とチェックサムを管理するため、適用済みファイルを書き換えず新しいファイルを追加する。

```sh
docker exec -i traojudge-postgres psql -U traojudge -d traojudge <<'SQL'
INSERT INTO users (id, display_name, revision)
VALUES ('user_example', 'サンプル利用者', 'revision_1');
SQL

curl http://127.0.0.1:3000/api/v1/users/user_example
curl http://127.0.0.1:3000/api/v1/users/missing
```

## テスト

DB不要のID検証とHTTPのエラー応答は通常のテストで実行する。

```sh
cargo test -p traojudge-core -p traojudge-test
```

PostgreSQLを使う結合テストは明示的に実行する。SQLxがテスト専用DBを作成・削除するため、接続ユーザーには `CREATE DATABASE` 権限が必要。`DATABASE_URL` はSQLxのテスト用で、アプリの `TRAOJUDGE_DATABASE_URL` とは別の環境変数。

```sh
DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge \
  cargo test -p traojudge-test --test users -- --include-ignored
```

空DBへのマイグレーションと再適用、公開プロフィールの取得・未設定値・不存在、DB制約を確認する。
