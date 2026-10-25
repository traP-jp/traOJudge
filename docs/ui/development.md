# UIの開発手順

コマンドは `UI/` で実行する。Node・npmは[依存管理](dependencies.md)の指定に合わせる。

## セットアップ

```sh
npm ci
npm run prepare
./scripts/gen-api.sh
```

`npm run prepare` はGitフックを設定する。同じリポジトリの他のworktreeにも適用される。

`gen-api.sh` は、通常問題・プロフィールなどの旧APIクライアントと、認証・コンテストで使う新APIの型を生成する。旧APIの生成にはJavaが必要。スキーマの参照先と生成器の管理は[依存管理](dependencies.md#github-actionsとapi生成)を参照。

新APIの型だけを更新する場合は `bash scripts/gen-api-types.sh` を実行する。`docs/API/front_back.yaml` から `src/api/generated/schema.ts` を生成する。

## APIの接続先

| 対象                    | 環境変数               | 設定場所                                                                                              |
| ----------------------- | ---------------------- | ----------------------------------------------------------------------------------------------------- |
| 通常問題などの旧API     | `VITE_API_BASE_URL`    | `.env.development` は `http://localhost:4010`、`.env.production` は `https://traojudge.trap.show/api` |
| 認証・コンテストの新API | `VITE_API_V1_BASE_URL` | 省略時は同一オリジンの `/api/v1`。指定する場合は `/api/v1` まで含める                                 |

接続先を変える場合は `.env.development.local` などで上書きする。Viteが環境変数を読み込むため、変更後は開発サーバーの再起動か再ビルドが必要。

Viteの開発サーバーは `/api/v1` を `http://localhost:4010` へ転送する。旧APIの設定は `src/api/config.ts`、新APIの共通クライアントは `src/api/client.ts` にある。

## 起動と検証

モックとフロントエンドは、同じcheckoutの `UI/` を開いた別々のターミナルで起動する。

```sh
npm run mock
```

モックは `127.0.0.1:4010` で待ち受け、設定ファイルの変更を自動で読み込む。停止は `Ctrl+C`。`./scripts/run-mock-api.sh` からも同じコマンドを実行できる。管理APIは無効にしてある。

フロントエンドを起動し、モックのCORS設定に合わせて `http://localhost:5173` を開く。

```sh
npm run dev
```

ログイン操作は[モックを使った仮ログイン](mock-login.md)、コンテストのサンプルURLは[トップと解説の確認手順](contest-top.md#モックで表示を確認する)を参照。モックは作成・更新内容を保存せず、提出コードの実際の採点も行わない。

ポートを変える場合は `npm run mock -- --port 4011` のように指定する。`.env.development.local` に次を設定し、Viteを再起動する。

```dotenv
VITE_API_BASE_URL=http://localhost:4011
VITE_API_V1_BASE_URL=http://localhost:4011/api/v1
```

検証は次の順に実行する。

```sh
npm run lint
npm run format:check
npm test
npm run test:lint
npm run build
```

`build` は型検査と本番用ビルドを実行する。`test:lint` は一時的な検査用ファイルを `src/` に置くため、型検査・buildとは並行実行しない。型検査だけなら `npm run type-check`、テストを監視実行するなら `npm run test:watch` を使う。
