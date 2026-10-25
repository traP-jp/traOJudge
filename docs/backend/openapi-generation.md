# OpenAPIからのRust型生成

HTTP入出力の型は `docs/API/front_back.yaml` から生成する。型の名前と共有範囲はOpenAPIで定義し、Rust側で生成型を手直ししたり、操作名由来の型に別名を付けたりしない。

## スキーマの名前と参照

`docs/API/schemas/*.yaml` に名前付きスキーマを定義し、`front_back.yaml` の `components.schemas` から参照する。各操作も同じスキーマを `$ref` で参照する。これにより、公開プロフィールは `Profile`、本人情報は `CurrentUser`、エラーは `Error` として生成され、複数の操作で共有される。

共通エラーの詳細は `ErrorDetail` として定義し、`Error.details` から参照する。異なる意味を持つデータを、形が同じという理由だけで同じスキーマにはしない。名前付きスキーマを追加するときは `components.schemas` への登録も追加する。parameters・responsesも、それぞれルートのcomponentsに登録する。

匿名のネストしたオブジェクトや `oneOf` の選択肢には、生成器が `ScopeOneOf1` などの補助型名を付ける。業務上の名前が必要な場合や複数箇所で使う場合は、独立した名前付きスキーマに切り出す。

## 生成と検証

Node.js・npm・Java・rustfmtを用意し、リポジトリのルートで実行する。

```sh
bash server/traojudge-backend_app/scripts/gen-api.sh
bash server/traojudge-backend_app/scripts/gen-api.sh --check
```

スクリプトはSwagger UIと同じ `deploy/swagger-ui/bundle.mjs` を使って、外部参照を内部参照へ束ねてからOpenAPI Generatorに渡す。名前付きcomponentsを保つことで、外部ファイルのスキーマが `GetMe200Response` などの操作別の型に展開されるのを防ぐ。bundlerの依存は既存のlockfileを使って一時ディレクトリにインストールし、終了時に削除する。

生成器のバージョンは `server/traojudge-backend_app/openapitools.json` で固定する。生成先は `src/models/`。HTTPコードからは `http::schema` 経由で参照できる。`--check` は再生成した結果との差分を検出し、ファイルを書き換えない。
