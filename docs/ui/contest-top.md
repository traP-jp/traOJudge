# コンテストtopの開発

`/contest/:contestId`に，大会の概要，開始・終了日時，開催状態，採点ルールを表示する．公開大会はログインせずに閲覧できる．非公開大会の閲覧権限はAPIで検証する．

## 概要はMarkdownで記述する

通常大会の`description`にMarkdown形式の概要を保存する契約とした．省略・空文字列の場合は，概要が未掲載であることを表示する．本文中のHTMLは実行しない．[概要テンプレート](./contest-overview-template.md)を執筆のたたき台に使える．

今回のUIは表示のみを扱う．解説一覧へのリンクを除き，編集画面や問題・質問・提出・順位表へのナビゲーションは，対応する画面の実装時に追加する．日時は閲覧端末の設定にかかわらず日本標準時で表示する．

## 大会の解説一覧を表示する

`/contest/:contestId/editorials`では，出題順に問題を並べ，問題ごとの公開解説を表示する．公開解説がない問題には未掲載であることを明示する．一覧から既存の解説本文画面に移動できる．本文画面はログインを必要とし，最新APIの`body`をMarkdownで表示する．

解説一覧はAPIが返す公開済みの解説を，開催中も表示する．大会全体の解説資料を取得するAPIはまだないため，この画面では扱わない．問題の題名を取得できない場合も，出題ラベルでどの問題か分かるようにする．

## APIクライアントを生成する

既存画面は旧契約の生成物を使っているため，コンテストtopのクライアントは`src/api/generated/contest/`へ分けて生成する．生成物は直接編集しない．`UI/`で次を実行する．

```sh
bash scripts/gen-contest-api.sh
```

入力は`docs/API/front_back.yaml`で，`getContest`，`listContestProblems`，`getProblem`，`listProblemEditorials`，`getEditorial`を生成対象とする．既存の`bash scripts/gen-api.sh development`からも呼び出される．

## モックで表示を確認する

コンテストAPIのバックエンド実装はまだないため，開発時はモックを使う．`UI/`でモックサーバーとViteを起動する．

```sh
bash scripts/run-mock-api.sh
npm run dev
```

`http://localhost:5173/contest/1`でサンプル大会を表示する．Viteは`/api/v1`へのリクエストを`localhost:4010`へ転送する．別のAPIに接続する場合は，`VITE_CONTEST_API_BASE_URL`に`/api/v1`まで含むURLを設定する．本番環境の既定値は同一オリジンの`/api/v1`である．

サンプル大会の解説一覧は`http://localhost:5173/contest/1/editorials`で確認する．AとCには解説，Bには未掲載の表示を用意した．
