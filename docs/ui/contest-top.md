# コンテストのトップと解説

コンテストのトップ・解説一覧・解説詳細を表示する。API生成と接続先の設定は[開発手順](development.md)を参照。

## ページと導線

| URL                                                               | 表示内容                   |
| ----------------------------------------------------------------- | -------------------------- |
| `/contest/:contestId`                                             | 概要、開催日時、採点ルール |
| `/contest/:contestId/editorials`                                  | 問題ごとの解説一覧         |
| `/contest/:contestId/problems/:problemId/editorials/:editorialId` | 解説本文                   |

各ページはコンテスト名・開催状態・残り時間のヘッダーを共有し、「トップ」「解説」のタブで移動できる。日時は日本標準時で表示する。

画面最上部のメニューにはコンテストへの入口がないため、現時点ではURLから開く。コンテストの問題・質問・提出・順位表への導線は、対応する画面の実装時に追加する。

## 表示と取得

公開コンテストのトップは未ログインでも表示する。現在のUIでは、終了後の公開解説も未ログインで閲覧できる。終了前の解説と非公開解説は、著者本人、または対象範囲の著者・運営ロールを持つ利用者に表示する。判定は [editorialAccess.ts](../../UI/src/utils/editorialAccess.ts) にある。

解説一覧はcursorをたどって取得する。一部の問題で取得に失敗しても、取得できた解説は表示し、再試行できる。解説詳細は、URLのコンテストに問題が含まれることと、解説の対象問題を確認する。

概要はコンテストの `description` をMarkdownとして表示する。省略・空文字列の場合は未掲載と表示する。[概要テンプレート](contest-overview-template.md)を利用できる。解説本文も共通の [MarkdownContent](../../UI/src/components/MarkdownContent.vue) で表示する。

## モックで表示を確認する

[共通の起動手順](development.md#起動と検証)に沿って起動し、次のURLを開く。

| サンプル             | URL                                                                  |
| -------------------- | -------------------------------------------------------------------- |
| 終了後・公開解説あり | <http://localhost:5173/contest/7e75693c-91da-50e8-920c-7f268c7d99c4> |
| 開催前               | <http://localhost:5173/contest/0a461c59-3ac1-56ab-8443-8aa7f64fa552> |
| 開催中               | <http://localhost:5173/contest/4bea2487-758f-5acc-b44b-2959fbdc887f> |

未ログインの表示を確認した後、[仮ログイン](mock-login.md)で著者としての表示を確認できる。サンプルと応答条件は [front-back-api-mock.json](../../UI/src/api/mock/front-back-api-mock.json) にある。
