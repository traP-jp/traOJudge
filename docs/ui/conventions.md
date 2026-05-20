# UI コーディング規約

このディレクトリは traO Judge のフロントエンド (`UI/`) を対象としたコーディング規約をまとめたものです。
**機械的に検証できる項目はすべて `UI/eslint.config.js` で強制** しています。このドキュメントは「なぜそのルールなのか」「lint で表現できない設計意図と運用」を残すためのものです。AI エージェントは lint が指摘した違反を回避しようとするのではなく、ここに書かれた意図に沿って書き直してください。

## スタック

- Vue 3 (Composition API) + TypeScript + Vite
- Pinia (状態管理), Vue Router (ルーティング)
- Tailwind CSS (スタイリング)
- ESLint + Prettier
- API クライアントは OpenAPI から自動生成

## ディレクトリ構成と責務

- `src/api/generated/` — OpenAPI から自動生成。**手で編集しない / lint 対象外**
- `src/api/mock/` — モックサーバー用スタブ
- `src/components/` — 再利用可能な UI 部品 (`Controls/`, `Navigations/` などにグルーピング)
- `src/composables/` — `useXxx` 形式の再利用ロジック (副作用・リアクティブ状態を含む)
- `src/stores/` — Pinia store。アプリ横断の状態とその更新ロジック
- `src/views/` — ルートに 1:1 対応するページ単位コンポーネント
- `src/router/` — ルーティング定義とナビゲーションガード
- `src/types/` — 複数ファイルから参照される共有型定義
- `src/utils/` — 純粋関数ユーティリティ (副作用なし、I/O なし)

責務が複数にまたがる場合は、所属を選ぶ前に「呼ぶ側はどこか」「副作用を持つか」を考えてください。

## 自動チェック

- `.claude/hooks/ui-format.sh` — Edit/Write 直後に Prettier → ESLint `--fix` を該当ファイルに実行
- `.claude/hooks/ui-typecheck.sh` — タスク完了時に `vue-tsc` を実行 (UI ファイルを編集したセッションのみ)
- `.husky/pre-commit` — コミット時に lint-staged (ESLint+Prettier) と `vue-tsc` を実行

エラーが出た場合は自動修正で済むものは直り、残ったエラーは AI にフィードバックされて再修正を促します。

---

## lint で強制している規約と意図

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

- **クラス順序を強制** (`tailwindcss/classnames-order`)
  - 順序統一で diff のノイズを減らし、共通スタイルの差分を見やすくする。
- **任意値 (`w-[317px]` 等) は warn** (`tailwindcss/no-arbitrary-value`)
  - 任意値はデザインシステム外の値が紛れ込んだサイン。やむを得ず使う場合は許容するが、まずは `tailwind.config.js` のトークンを使えないか確認する。

---

## lint で表現できない設計方針

### API クライアント

- **`src/api/generated/` を直接 import して使う**
- **ラッパー層やストア経由を新規導入しない**
  - 早すぎる抽象は将来の API 変更時に「ラッパー側も追従させる」二重作業を生む。必要になった時点で設計し直す方針。
- スキーマ変更時は **ルートディレクトリ** で `./scripts/gen-api.sh development` を実行して再生成
- 開発環境では `./scripts/run-mock-api.sh` でモックサーバーを `localhost:4010` に立てる

### 型の書き方バランス

- 型は **推論を優先**。冗長な明示は避ける (`const x: number = 1` のような明示は不要)
- ただし関数の引数・戻り値・公開 API は **明示**して契約を読み手に伝える
- lint で判定不可な主観領域なのでレビュー時の議論で揃える

### Tailwind の使い分け

- `tailwind.config.js` にあるカラー/スペーシングを優先して使う
- 既存トークンで表現できない場合のみ任意値 (`w-[317px]`) を使い、頻出するならトークン化を検討する

### コメント

- WHAT (何をしているか) ではなく WHY (なぜそうしているか) を書く
- やむを得ず `<style scoped>` を導入する場合や `eslint-disable` を使う場合は、**なぜその例外が必要か** をコメントで残す
