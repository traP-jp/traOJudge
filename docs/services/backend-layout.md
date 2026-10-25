# バックエンドのコード構成

バックエンドはクレートごとに責務を分ける。外部サービスとの接続は[AWS側の構成ブロック](aws-building-blocks.md)に対応するが、ブロックごとにクレートやプロセスを分けるものではない。

```text
server/
├── traojudge-core/src/
│   ├── domain/
│   │   ├── model/        業務モデル・状態遷移
│   │   └── service/      モデルをまたぐ業務ルール
│   ├── application/      提出確定・結果受理などのユースケース
│   └── ports/
│       └── repository/   永続化・トランザクションの契約
├── traojudge-infra/src/
│   ├── postgres/         DB操作・トランザクション・migration
│   ├── s3/               ファイル転送・検証・版固定
│   └── sqs/              メッセージの送受信・可視性管理・削除
├── traojudge-backend_app/src/
│   ├── bin/              実行エントリポイント
│   ├── bootstrap/        設定・依存の組み立て・起動
│   ├── http/             HTTPの受付・入力と応答の変換
│   ├── consumers/        結果メッセージの受付
│   └── models/           OpenAPIから生成するHTTP入出力型
├── traojudge-workflow/src/
│   └── storage/          ワークフローエンジンの永続化・Unit of Workの契約
├── traojudge-test/        クレートをまたぐ結合確認
└── workspace-hack/        cargo-hakari が管理するビルド用の依存関係
```

## 依存と処理の置き場所

依存関係の feature 統一には cargo-hakari を使う。再生成と検証の手順は [Rust の依存関係管理](rust-development.md) を参照する。

- `core`は`infra`・`backend_app`に依存しない。`infra`は`core`、`backend_app`は`core`と`infra`に依存する。
- `domain`は外部I/Oに依存しない業務ルールを扱う。`application`はdomainとportsを使ってユースケースを組み立て、操作権限の確認とトランザクション境界を担う。
- `ports`は必要になった外部操作のtraitと入出力型を定義する。永続化以外のファイル・外部通信の契約もここに追加する。SQL・AWS SDK・HTTPの型を契約に含めない。
- `infra`はportsの実装を提供する。業務上の再試行や後続ノードの選択はapplicationが判断する。
- `http`と`consumers`は入力をapplicationへ渡す。生成済みの`models`はHTTP境界で使い、Coreには持ち込まない。`http::schema`からも参照できる。[OpenAPIからのRust型生成](../backend/openapi-generation.md)に従い、型名・共有範囲はスキーマ側で定義する。
- `bootstrap`で具体的なアダプターとユースケースを組み立て、HTTPや受信処理を起動する。

## ワークフローのStorage backend

`traojudge-workflow::storage`に、backend実装が満たす`UnitOfWorkProvider`と`UnitOfWork`のtraitを置く。
`UnitOfWorkProvider::begin(&self)`が非同期にUnit of Workを開始し、`commit(self)`で変更を確定、`rollback(self)`で破棄する。
確定・破棄は所有権を消費するため、呼び出しが失敗した場合も同じUnit of Workは再利用できない。
`commit`のエラーは変更が確実に破棄されたことを意味しない。確定結果が不明になる場合の復旧は、具体的なbackendとユースケースを実装するときに定義する。

Unit of Workの関連型にはproviderの借用期間を持たせ、backendが保持する接続などを借用できるようにする。
providerは`Send + Sync`、Unit of Workと各操作のfutureは`Send`とする。エラー型はbackendが定義し、開始・確定・破棄で共有する。
未確定のUnit of Workが破棄された場合、backend実装は変更が確定されないことを保証する。非同期の破棄処理を明示的に待つ場合は`rollback`を呼ぶ。
現在は契約のみで、具体的なbackendやリポジトリ操作はまだ提供しない。

## 状態更新の境界

結果の受理、ノード状態の更新、実行可能になった後続ノードの送信待ち登録は、applicationが同じDBトランザクションで確定する。トランザクションの契約はports、DB上の実装はinfraに置く。SQSへの送信は送信待ちの記録から行い、DBトランザクション中の外部送信を前提にしない。

重複結果と並行更新による二重反映を防ぐ制約はDB側にも設ける。具体的な契約、スキーマ、再試行・期限切れからの復旧は実装時に定義する。ジャッジ結果の受理・送信待ちなどのapplication・ports・infra・consumersは配置先を用意した段階であり、これらの機能の実装完了を意味しない。

[利用者の公開プロフィール](../backend/users.md)では、Coreの取得処理・PostgreSQLのRepositoryとマイグレーション・HTTPを実装している。
[本人情報とアカウント管理](../backend/me.md)では、セッション認証・本人プロフィール更新・認証手段管理を実装している。メール配送・外部OAuthは接続契約とテスト用アダプターまでを扱う。

アカウント管理では `core::ports::repository::account` の `AccountUnitOfWorkProvider` / `AccountUnitOfWork` を使う。業務判断・永続化操作の組み合わせ・commitはapplication、本人に限定したSQL・行ロック・未確定時のrollbackはinfraが担当する。メール配送やOAuth照会の間はUoWを保持しない。HTTPのmiddleware・extractor・生成DTOはHTTP層に置く。
