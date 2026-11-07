# コンテストtopの開発

`/contest/:contestId`に，大会の概要，開始・終了日時，開催状態，採点ルールを表示する．公開大会はログインせずに閲覧できる．非公開大会の閲覧権限はAPIで検証する．

## 概要はMarkdownで記述する

通常大会の`description`にMarkdown形式の概要を保存する契約とした．省略・空文字列の場合は，概要が未掲載であることを表示する．本文中のHTMLは実行しない．[概要テンプレート](./contest-overview-template.md)を執筆のたたき台に使える．

今回のUIは表示のみを扱う．大会タイトルの下に「トップ」と「解説」のタブを置き，Vue Routerで画面を切り替える．問題・質問・提出・順位表のタブは，対応する画面の実装時に追加する．日時は閲覧端末の設定にかかわらず日本標準時で表示する．

## 大会の解説一覧を表示する

`/contest/:contestId/editorials`では，出題順に問題を並べ，閲覧権限のある解説を表示する．解説がない問題には未掲載であることを明示する．一覧から既存の解説本文画面に移動できる．本文画面はログインを必要とし，最新APIの`body`をMarkdownで表示する．

大会終了前は，一般利用者に解説を表示しない．対象大会の`contest-admin`または`contest-setter`はすべての解説を閲覧できる．解説の`authorId`に一致する著者と`editiorial-author`は，担当する解説を閲覧できる．画面は`getContest`の終了時刻，`listMyRoles`の対象範囲付きロール，ログイン中の本人IDを使い，閲覧を許可しない解説を表示しない．大会終了後は公開解説を表示する．終了時刻になると画面を再読み込みする．

画面の表示制御だけでは直接のAPI要求を防げない．`listProblemEditorials`と`getEditorial`も，サーバー時刻，公開状態，対象大会，対象範囲付きロールで閲覧権限を判定する契約とする．現在のバックエンドには解説取得処理がないため，この認可は実装時に必要となる．大会全体の解説資料を取得するAPIはまだないため，この画面では扱わない．問題の題名を取得できない場合も，出題ラベルでどの問題か分かるようにする．

## APIクライアントを生成する

既存画面は旧契約の生成物を使っているため，コンテストtopのクライアントは`src/api/generated/contest/`へ分けて生成する．生成物は直接編集しない．`UI/`で次を実行する．

```sh
bash scripts/gen-contest-api.sh
```

入力は`docs/API/front_back.yaml`で，`getContest`，`listContestProblems`，`getProblem`，`listProblemEditorials`，`getEditorial`，`listMyRoles`を生成対象とする．既存の`bash scripts/gen-api.sh development`からも呼び出される．

## モックで表示を確認する

コンテストAPIのバックエンド実装はまだないため，開発時はモックを使う．`UI/`でモックサーバーとViteを起動する．

```sh
bash scripts/run-mock-api.sh
npm run dev
```

`http://localhost:5173/contest/1`でサンプル大会を表示する．Viteは`/api/v1`へのリクエストを`localhost:4010`へ転送する．別のAPIに接続する場合は，`VITE_CONTEST_API_BASE_URL`に`/api/v1`まで含むURLを設定する．本番環境の既定値は同一オリジンの`/api/v1`である．

終了済みのサンプル大会の解説一覧は`http://localhost:5173/contest/1/editorials`で確認する．AとCには解説，Bには未掲載の表示を用意した．開催前の表示は`http://localhost:5173/contest/2/editorials`で確認する．モックは権限判定を行わないため，認可の検証には使用しない．
