# アーキテクチャ

クリーンアーキテクチャを参考に構成

## パッケージ構成

```bash
.
├── bin/main.rs # エントリーポイント
├── di.rs # 依存性注入
│
├── domain # 他層に依存しない
│  ├── entities # 他層に依存しないドメインオブジェクト、を格納する
│  ├── external # 外部APIに関するトレイトの定義
│  └── repository # リポジトリ操作に関するトレイトの定義
│
├── infrastructure # 外部APIやDBへのアクセス (domain層に依存)
│  ├── external # 外部APIへのアクセスの実装
│  └── repository # DBへのアクセスの実装
│
├── usecase # ? todo
│
└── presentation # ? todo
```
