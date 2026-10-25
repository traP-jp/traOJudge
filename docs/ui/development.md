# UIの開発手順

コマンドは `UI/` で実行する。Node・npmは[依存管理](dependencies.md)の指定に合わせる。

## セットアップ

```sh
npm ci
npm run prepare
./scripts/gen-api.sh
```

`npm run prepare` はGitフックを設定する。同じリポジトリの他のworktreeにも適用される。

API生成にはJavaが必要。`gen-api.sh` は通常問題とコンテストトップのクライアントを生成する。スキーマの参照先と生成器のバージョン管理は[依存管理](dependencies.md#github-actionsとapi生成)を参照。

## APIの接続先

| 対象                | 環境変数                    | 設定場所                                                                                              |
| ------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------- |
| 通常問題などの旧API | `VITE_API_BASE_URL`         | `.env.development` は `http://localhost:4010`、`.env.production` は `https://traojudge.trap.show/api` |
| コンテストAPI       | `VITE_CONTEST_API_BASE_URL` | 省略時は `/api/v1`。詳細は[コンテストトップの開発](contest-top.md)                                    |

接続先を変える場合は `.env.development.local` などで上書きする。Viteが環境変数を読み込むため、変更後は開発サーバーの再起動か再ビルドが必要。

旧APIには、アプリ起動時に `src/api/config.ts` で接続先と `credentials: include` を設定する。

## 起動と検証

モックを使う場合は、別のターミナルで起動する。

```sh
npm run mock
```

モックは `127.0.0.1:4010` で待ち受け、設定ファイルの変更を自動で読み込む。停止は `Ctrl+C`。`./scripts/run-mock-api.sh` からも同じコマンドを実行できる。管理APIは無効にしてある。

フロントエンドを起動し、モックのCORS設定に合わせて `http://localhost:5173` を開く。

```sh
npm run dev
```

モック接続時は、ログイン画面で形式の正しいメールアドレスとパスワードを送るとCookieが設定され、ログアウトで削除される。問題の作成・更新内容は保存されず、再取得すると別の仮データになる。コンテストトップのサンプルは `/contest/1` で確認できる。

ポートを変える場合は `npm run mock -- --port 4011` のように指定する。UI側も `VITE_API_BASE_URL` と `VITE_CONTEST_API_BASE_URL` をそのポートに合わせる。

```sh
npm run lint
npm run format:check
npm test
npm run test:lint
npm run build
```

`build` は型検査と本番用ビルドを実行する。型検査だけなら `npm run type-check`、テストを監視実行するなら `npm run test:watch` を使う。
