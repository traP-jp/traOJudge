# コンテストtopの開発

`/contest/:contestId`に，大会の概要，開始・終了日時，開催状態，採点ルールを表示する．公開大会はログインせずに閲覧できる．非公開大会の閲覧権限はAPIで検証する．

## 概要はMarkdownで記述する

通常大会の`description`にMarkdown形式の概要を保存する契約とした．省略・空文字列の場合は，概要が未掲載であることを表示する．本文中のHTMLは実行しない．[概要テンプレート](./contest-overview-template.md)を執筆のたたき台に使える．

今回のUIは表示のみを扱う．編集画面や，問題・質問・提出・順位表へのナビゲーションは，対応する画面の実装時に追加する．日時は閲覧端末の設定にかかわらず日本標準時で表示する．

## APIクライアントを生成する

既存画面は旧契約の生成物を使っているため，コンテストtopのクライアントは`src/api/generated/contest/`へ分けて生成する．生成物は直接編集しない．`UI/`で次を実行する．

```sh
bash scripts/gen-contest-api.sh
```

入力は`docs/API/front_back.yaml`で，`getContest`を生成対象とする．既存の`bash scripts/gen-api.sh development`からも呼び出される．

## モックで表示を確認する

コンテストAPIのバックエンド実装はまだないため，開発時はモックを使う．[共通の起動手順](development.md#起動と検証)に沿って，モックサーバーとViteを別々のターミナルで起動する．

`http://localhost:5173/contest/1`でサンプル大会を表示する．Viteは`/api/v1`へのリクエストを`localhost:4010`へ転送する．別のAPIに接続する場合は，`VITE_CONTEST_API_BASE_URL`に`/api/v1`まで含むURLを設定する．本番環境の既定値は同一オリジンの`/api/v1`である．
