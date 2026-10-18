# バックエンドのコード構成

バックエンドは既存の3クレートに責務を分ける。外部サービスとの接続は[AWS側の構成ブロック](aws-building-blocks.md)に対応するが、ブロックごとにクレートやプロセスを分けるものではない。

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
└── traojudge-test/        クレートをまたぐ結合確認
```

## 依存と処理の置き場所

- `core`は`infra`・`backend_app`に依存しない。`infra`は`core`、`backend_app`は`core`と`infra`に依存する。
- `domain`は外部I/Oに依存しない業務ルールを扱う。`application`はdomainとportsを使ってユースケースを組み立て、操作権限の確認とトランザクション境界を担う。
- `ports`は必要になった外部操作のtraitと入出力型を定義する。永続化以外のファイル・外部通信の契約もここに追加する。SQL・AWS SDK・HTTPの型を契約に含めない。
- `infra`はportsの実装を提供する。業務上の再試行や後続ノードの選択はapplicationが判断する。
- `http`と`consumers`は入力をapplicationへ渡す。生成済みの`models`はHTTP境界で使い、Coreには持ち込まない。`http::schema`からも参照できる。生成先は既存のスクリプトに合わせて維持する。
- `bootstrap`で具体的なアダプターとユースケースを組み立て、HTTPや受信処理を起動する。

## 状態更新の境界

結果の受理、ノード状態の更新、実行可能になった後続ノードの送信待ち登録は、applicationが同じDBトランザクションで確定する。トランザクションの契約はports、DB上の実装はinfraに置く。SQSへの送信は送信待ちの記録から行い、DBトランザクション中の外部送信を前提にしない。

重複結果と並行更新による二重反映を防ぐ制約はDB側にも設ける。具体的な契約、スキーマ、再試行・期限切れからの復旧は実装時に定義する。現在のapplication・ports・infra・consumersは配置先を用意した段階であり、これらの機能の実装完了を意味しない。
