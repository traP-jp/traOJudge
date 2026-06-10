# traOJudge — AI エージェント向けエントリポイント

このリポジトリで作業する際は、対象領域ごとに以下のドキュメントを **着手前に必ず読む** こと。
ドキュメントの本体は `docs/` に集約されています。このファイルは入口 (スタブ) です。

## 領域別エントリ

- **UI (フロントエンド / `UI/`)**: [`docs/ui/conventions.md`](./docs/ui/conventions.md)
  - Vue 3 + TS + Tailwind のコーディング規約と設計意図。
- **API server (バックエンド / `server/`)**: [`docs/apiserver/conventions.md`](./docs/apiserver/conventions.md)
  - Rust + Axum のコーディング規約、crate 責務、DB/API 境界の設計意図。
- **API スキーマ**: [`docs/API/front_back.yaml`](./docs/API/front_back.yaml)
- **サービス構成**: [`docs/services/architecture-overview.puml`](./docs/services/architecture-overview.puml)

## 共通の作法

- 機械的に検証可能な規約は各領域の lint / formatter / 型チェックで強制している。lint 指摘を回避するためだけの書き換えはせず、`docs/` の本文に書かれた **なぜ** に沿って書き直すこと。
- ドキュメントを追加・更新する場合は `docs/<topic>/` 配下に置き (AI 向け / 人間向けで分けず topic で切る)、必要なら本ファイル (または該当サブプロジェクトの `CLAUDE.md`) からリンクを足す。AI 専用ファイルは作らない。
- サブプロジェクト (`UI/` 等) 直下に `CLAUDE.md` を置く場合も中身はスタブのみとし、本体は `docs/` を参照させる。
