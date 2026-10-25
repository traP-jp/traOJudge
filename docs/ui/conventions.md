# UI コーディング規約

`UI/` のコーディング規約と設計方針をまとめる。起動と検証は[開発手順](development.md)を参照。

## スタック

- Vue 3 (Composition API) + TypeScript + Vite
- Pinia (認証・クライアント状態), TanStack Vue Query (サーバー状態), Vue Router (ルーティング)
- Tailwind CSS (スタイリング)
- ESLint + Prettier
- APIの型はOpenAPIから生成。旧APIは生成クライアント、新APIはopenapi-fetchを使う

## ディレクトリ構成と責務

| 配置               | 内容                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------- |
| `src/api/`         | APIの接続設定とQueryClient。`generated/` はスキーマから再生成するコード、`mock/` はモックの定義 |
| `src/components/`  | ページを構成するUI部品                                                                          |
| `src/composables/` | `useXxx` 形式の取得・操作・リアクティブ状態の処理                                               |
| `src/stores/`      | アプリで共有する認証状態と、その更新処理                                                        |
| `src/views/`       | ルートに対応するページ                                                                          |
| `src/router/`      | ルート定義、認証ガード、URL条件と遷移先                                                         |
| `src/types/`       | 複数ファイルで使う型                                                                            |
| `src/utils/`       | 副作用やI/Oを持たない関数                                                                       |

### componentsとcomposablesの配置

部品の配置は、扱う機能と役割で決める。複数のページで使っていても、問題や提出の知識を持つ部品は、その機能のディレクトリに置く。

| components内の配置                                              | 内容・例                                                                                 |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 直下                                                            | 機能を問わず使う表示部品。`ListingTable`、`QueryState`、`MarkdownContent`など            |
| `controls/`                                                     | ボタン・入力・メニュー操作。テキスト入力は `textbox/`、ページ送りは `pagination/`        |
| `icons/`・`editor/`                                             | アイコンとコードエディター。エディター用のハイライト処理も `editor/` に置く              |
| `navigation/`                                                   | 共通のナビゲーションとサイドメニューの枠                                                 |
| `contests/`・`problems/`・`submissions/`・`users/`・`settings/` | 機能固有の部品。コンテストのヘッダー、サイドメニューの項目定義、`ProblemSubmitForm` など |

composablesは `contests/`、`problems/`、`submissions/`、`users/` に機能別の処理を置く。機能をまたいで使う `useApiQuery`、`useSessionQuery`、`useMarkdownRenderer` は直下に置く。URLを扱う `useListRoute` と `useRouteId` は `router/` にまとめる。テストは対象の実装と同じディレクトリに置く。

## 自動チェック

| フック                          | 実行条件                                                             | 処理                                        |
| ------------------------------- | -------------------------------------------------------------------- | ------------------------------------------- |
| `.claude/hooks/ui-format.sh`    | Claude CodeでUI内のVue・TypeScript・JavaScriptファイルを編集した直後 | ESLint `--fix` → Prettier。生成物などは除く |
| `.claude/hooks/ui-typecheck.sh` | 上記の編集があったセッションの完了時                                 | `vue-tsc`                                   |
| `.husky/pre-commit`             | `UI/` 配下の変更がステージされているコミット時                       | lint-staged → `vue-tsc`                     |

---

## lint・formatterで確認する規約と意図

### Vue コンポーネント

- **`<script setup lang="ts">` を必須化** (`vue/component-api-style`, `vue/block-lang`)
  - Options API と Composition API の混在はレビュー時の認知コストが高い。書き方を一本化することで読み手はパターンマッチに集中できる。`<script>` を JS で書くと型の恩恵を失うため `lang="ts"` も必須。
- **`defineProps` / `defineEmits` は型ベース宣言のみ** (`vue/define-props-declaration`, `vue/define-emits-declaration`)
  - ランタイム宣言 (`{ type: String, required: true }`) は TS 型と二重管理になり、片方を更新し忘れた瞬間に乖離する。型ベースなら TS 型が Single Source of Truth になる。
  ```ts
  // OK
  const props = defineProps<{ userId: string; isActive?: boolean }>()
  const emit = defineEmits<{ (e: 'select', id: string): void }>()
  ```
- **`<style>` ブロック禁止** (`vue/no-restricted-block`)
  - スタイルは Tailwind に集約してデザイントークン (色 / 間隔 / タイポ) を一元管理する。`<style scoped>` はクラス衝突回避の動機で増えがちだが、Tailwind を使えば衝突しないため不要。スタイル方針が二系統に分裂するのを防ぐ。
- **SFC ブロック順序: `<script setup>` → `<template>`** (`vue/block-order`)
  - 「先にロジック、次にテンプレート」の順で読む方が概念フローとして自然。

### TypeScript

- **`any` 禁止** (`@typescript-eslint/no-explicit-any`)
  - `any` は型システムを部分的に無効化し、リファクタ時に静かにバグを通す。未知の型を扱いたいときは `unknown` を使い、絞り込みを強制する。
- **`../foo` 形式の親相対 import 禁止** (`no-restricted-imports`)
  - ファイル移動でリンクが切れる。読み手も深さが直感的に分からない。`@/` エイリアスは `src` ルートからの絶対参照で安定し grep もしやすい。

### 命名

- **`.vue` は PascalCase / `.ts` は camelCase** (`unicorn/filename-case`)
  - Vue コンポーネント以外で PascalCase ファイルを作ると、ファイル名から「これはコンポーネントではない」が一目で分かる利点を失う。逆も同様。

### コード品質

- **ネストした if 禁止** (`no-restricted-syntax`)
  - 条件の組み合わせが見えにくく読み手の認知負荷が高い。早期 return / ガード節 / 関数分割で平坦化し、各分岐の責務を明確にする。`else if` チェーンは縦深さが増えないため許可。
- **コールバックはアロー関数で書く** (`prefer-arrow-callback`)
  - `function` 式は独自の `this` を持つため、Composition API や非同期処理で `this` を意図せずシャドウする事故が起きやすい。アロー関数は `this` を字句的に継承するためこの種のバグを構造的に防げる。auto-fix 対応。
- **`eslint-disable` には必ず理由を書く** (`@eslint-community/eslint-comments/require-description`)
  - 理由なし disable が蓄積するとルール自体への信頼が薄れ、ルール見直しの判断もできなくなる。

### Tailwind

- **クラス順序はPrettierで整える** (`prettier-plugin-tailwindcss`)
  - `tailwindcss/classnames-order` は無効にし、クラス順の整形をPrettierに集約している。
- **任意値 (`w-[317px]` 等) は warn** (`tailwindcss/no-arbitrary-value`)
  - `npm run lint` は `--max-warnings=0` のため、警告が残ると失敗する。まずは `UI/src/assets/main.css` の `@theme` にあるトークンを使えないか確認する。任意値が必要な場合は、対象箇所に理由付きの `eslint-disable` を置き、このルールだけを抑制する。

---

## lint で表現できない設計方針

### 状態の持ち方

| 状態                                | 管理する場所                               |
| ----------------------------------- | ------------------------------------------ |
| 認証状態・本人情報                  | Piniaのstore                               |
| 共通処理へ移行した画面のAPI取得結果 | Vue Queryと機能別composable                |
| 一覧のページ番号・難易度条件        | URL。読み書きは `router/listConditions.ts` |
| フォームの入力・メニューの開閉      | その状態を使うページや部品                 |

Queryの取得結果を編集するフォームでは、入力用の状態を別に持つ。再取得によって入力中の内容を置き換えないためである。

### API取得と表示

問題詳細・問題一覧・提出一覧・ユーザー詳細・コンテストとその解説の取得は、機能別composableを使う。API呼び出し、query key、取得条件をそこで定義する。問題編集・設定画面・言語一覧取得には、生成APIを直接呼ぶ既存実装が残っている。

認証・コンテストの通信には `api/client.ts` の `apiClient` と、`api/generated/schema.ts` の生成型を使う。HTTPエラーは共通クライアントで `ApiError` にし、本文が必須の応答には `requireData` を使う。通常問題などの旧APIは、既存の生成クライアントを使う。

対象IDや条件はrefまたはgetterで渡す。初回の値だけを渡すと、画面内で対象を切り替えたときに取得が追従しない。機能別composableでは、共通Queryが渡す `signal` をAPI呼び出しへ引き渡す。

画面は返された `view` を表示し、再取得には `refresh` を使う。共通の読み込み・エラー表示には `QueryState` を使う。401ではセッションを破棄し、403・404では古いデータを隠す。同じ対象の再取得が通信障害などで失敗した場合は、取得済みデータに警告を添えて表示する。

`useApiQuery` は公開取得と認証必須取得を扱う。認証必須の取得では `requiresAuth: true` を指定する。`useSessionQuery` は旧API向けに認証必須の設定とquery keyの接頭辞を固定したもの。公開APIは未ログイン時に `credentials: 'omit'` で取得する。[useContestQuery](../../UI/src/composables/contests/useContestQuery.ts) を参照。

### 認証と画面遷移

認証は `stores/user.ts`、認証に応じた画面遷移はrouterが扱う。ストアは新APIの `CurrentUser` を保持する。初期化ではCookieのセッションを確認し、通信失敗後は再試行できる。セッション切り替え時には進行中の取得を中断し、Queryのキャッシュを破棄する。前のセッションから遅れて届いた応答は反映しない。

認証必須ページは、ログイン状態とルートの権限を確認してから表示する。公開ページは認証確認の完了を待たずに表示し、閲覧者が変わったらデータを取得し直す。通常問題の開発用の操作許可には `composables/problems/useMockAuthGuard.ts` を使う。操作方法と制約は[モックを使った仮ログイン](mock-login.md)を参照。

### 一覧と提出

一覧条件の変更には `useListRoute` の `updateConditions` を使う。難易度変更とページ番号のリセットを一度の遷移で反映する。対象IDは `useRouteId`、問題・ユーザーへの遷移先は `router/locations.ts` の関数を使う。

フィルタは `QueryState` の外に置き、再取得中も開いたメニューを維持する。表には行と表示設定を渡し、ページ送りは表の外で扱う。旧APIの番号式ページ送りには `NumberedPagination` を使う。問題一覧の難易度絞り込みは、旧APIの制約により取得したページ内で行う。

提出は `useSubmitSolution` が送信時の対象と入力を保持し、成功後に問題別・本人別の提出一覧を再取得の対象にする。フォームは、その後の画面遷移と入力状態を扱う。

実装例は[問題一覧](../../UI/src/views/ProblemsView.vue)、[ユーザーの提出一覧](../../UI/src/views/user/UserSubmissions.vue)、[提出フォーム](../../UI/src/components/submissions/ProblemSubmitForm.vue)を参照。

### Markdown

問題文・コンテスト概要・解説・編集プレビューには `MarkdownContent` を使う。`useMarkdownRenderer` で数式とコードの色分けを扱い、本文中のHTMLは無効にしている。生成したHTMLはDOMPurifyでサニタイズする。コードの色分けを初期化できなくても本文を表示し、次に開いたときに再試行する。

### 型の書き方バランス

- 型は **推論を優先**。冗長な明示は避ける (`const x: number = 1` のような明示は不要)
- ただし関数の引数・戻り値・公開 API は **明示**して契約を読み手に伝える
- lint で判定不可な主観領域なのでレビュー時の議論で揃える

### Tailwind の使い分け

- `UI/src/assets/main.css` の `@theme` にあるカラー/スペーシングと、Tailwind標準の数値ユーティリティを優先して使う
- 既存トークンで表現できない場合のみ任意値 (`w-[317px]`) を使い、頻出するならトークン化を検討する

### コメント

- WHAT (何をしているか) ではなく WHY (なぜそうしているか) を書く
- やむを得ず `<style scoped>` を導入する場合や `eslint-disable` を使う場合は、**なぜその例外が必要か** をコメントで残す
