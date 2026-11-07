# 新定義のモックAPIを確認する

UI開発者向けの手順である．モックと生成クライアントは，どちらも[front_back.yaml](../API/front_back.yaml)の現行定義を使う．初回は起動手順に従い，変更時は末尾の検証コマンドを実行する．

## クライアントを生成し，モックを起動する

`UI/`で次を実行する．生成にはNode.js，Java，モックの起動にはDockerが必要になる．

```sh
npm ci
bash scripts/gen-api.sh development
bash scripts/run-mock-api.sh
npm run dev
```

`http://localhost:5173/login`でモックログインを行い，`/problems`で問題を選ぶ．問題の作成は`/problems/create`，プロフィールの編集は`/settings/profile`で確認する．コンテストのサンプルURLは[順位表ページの開発](./standings.md)に記載している．

APIの既定パスは開発・本番ともに`/api/v1`である．Viteは`/api/v1`とファイル転送用の`/mock-storage`を`localhost:4010`へ転送する．`MOCK_API_TARGET`で転送先，`VITE_API_BASE_URL`でAPIのベースURLを変更できる．コンテスト画面の既存設定`VITE_CONTEST_API_BASE_URL`も引き続き優先される．モックが許可するOriginは`http://localhost:5173`である．

生成先は`src/api/generated/`に統一した．`gen-auth-api.sh`と`gen-contest-api.sh`は互換用の入口として残し，同じ生成処理を呼ぶ．

## 一覧・更新・提出は新しい契約に従う

| 対象 | モックと画面の動作 |
| --- | --- |
| 問題一覧・解説一覧・自分の提出 | `items`と`nextCursor`を使ってページを移動する．総件数やoffsetは使わない． |
| 問題の作成・編集 | `title`と`statement`を送る．定義にない難易度・実行時間上限・メモリ上限の入力欄は表示しない． |
| 問題・解説・プロフィールの更新 | GETで受け取ったETagをIf-Matchへ渡してPATCHする．未指定は428，古い値は412になる． |
| 解説の作成 | `POST /editorials`へ`problemId`，`title`，`body`を送る． |
| プロフィール | `displayName`，`bio`，GitHub・AtCoder・Xの自己申告IDを扱う．未設定のIDは空文字列になる． |
| 練習提出 | UTF-8のバイト数とSHA-256を送って転送先を取得し，指定URLへPUTした後，`uploadId`で提出を受け付ける．採点結果はJudgeRunから取得する． |
| テストケース | 問題配下で取得し，入力・期待出力のファイル登録状況を表示する． |

問題・解説・プロフィールの変更と提出は，モックのメモリに保持する．再起動すると初期状態へ戻る．ファイル実体の保管や採点は行わず，提出結果は固定のACを返す．トークン・Cookieによる認証，役割ごとの認可，冪等キーによる重複排除は再現しない．モックはこれらの検証には使えない．

問題の作成者に編集権限が付くとは限らない．問題設定への導線は`GET /me/problems`で確認し，初期サンプル3問だけを作業可能な問題として返す．新規作成時の編集権限の自動付与は，定義上まだ決まっていない．

全員の提出や他人の提出一覧を取得するAPIはないため，提出一覧には自分の提出を表示する．問題別の一覧は自分の提出を問題IDで絞り込む．

## 未決の操作と未定義APIを区別する

認可条件が`pending`の問題公開，解説の削除・公開状態変更，テストケースの追加・変更・削除は，定義どおり`403 policy_not_decided`を返す．画面では準備中と表示する．問題を非公開にする操作は定義済みで，service-adminの利用者にだけボタンを表示する．

順位表の`/standings/details`，`/standings/team/details`，`/standings/extended`は定義がないため，従来の仮応答を維持する．正式APIのサンプルと同じ対象を参照できるよう，識別子だけUUIDへ揃えている．

## 定義と動作の両方を検証する

```sh
npm test
npm run test:mock
npm run build
```

`npm test`は，モックの経路，静的な応答・初期データ，要求検証用スキーマ，宣言済みレスポンスヘッダーをOpenAPIと照合し，画面の単体テストを実行する．

`npm run test:mock`は専用のDockerコンテナを空いているポートで起動する．検索・ページ送り，更新競合，保存後の取得，提出・採点結果取得をHTTPで確認し，終了時にコンテナを停止する．普段使っているモックのデータは変更しない．CIでも同じ検証を実行する．

ETagはリソースの版を示すヘッダーで，If-Matchは更新対象の版を指定するヘッダーである．カーソルは次ページを取得するための文字列で，画面では内容を解釈せずAPIへ渡す．定義と画面の対応で判断に迷う場合は，[API定義](../API/front_back.yaml)と[UI規約](./conventions.md)を確認する．
