# traOJudge 要求仕様

対象は、traOJudge のリリース時点で満たす要件である。現在の実装の説明ではなく、リリースに向けたシステム全体の分割、責務、合意済みの要求を記述する。

全章に本文を記載し、未決事項は `TODO` として残す。具体的なスタック、インスタンス、費用などを未決のまま確定事項として扱わない。

## システムの要素分割

以下は責務を整理するための論理的な分割である。要素ごとに別サービス・別プロセスを配置することまでは要求しない。

| 要素 | 責務 |
| --- | --- |
| WebUI | 公開情報の閲覧、作問、提出、コンテスト参加・運営の操作画面 |
| API | クライアントから操作を受け付け、各機能・基盤へ処理をつなぐ公開インターフェース |
| 利用者・認証・権限 | 利用者の登録、ログイン、認証状態、ロールに基づく操作権限 |
| 問題・作問・解説 | 問題、テストケース、judge-definition、解説、共同作業者の管理 |
| コンテスト・順位 | 通常・バーチャルコンテスト、参加、開催ルール、Clar、得点と順位 |
| 提出・ジャッジ管理 | 提出の受付、ファイル参照、ジャッジ要求、結果の記録、再ジャッジ |
| ジャッジ実行基盤 | 必要ファイルの取得、ジャッジプログラムの実行、得点・判定結果の報告 |
| データベース | 利用者、問題、コンテスト、提出・結果などの構造化データの永続化 |
| ファイル保管 | 提出ソースと作問ファイルの実体の保存・取得 |
| ジョブキュー | ジャッジ要求と結果の非同期受け渡し |
| メール送信 | マジックリンクの送信要求の受け取りと配送 |
| 作問クライアント | Writer がファイルなどを登録するためのツール。既存構成図の TraoPy が該当 |

配置と通信は[製品の位置づけ](08-product-perspective/product-perspective.md)、責務と要求の対応は[要求事項の割り当て](13-apportioning-of-requirements/apportioning-of-requirements.md)に記す。

## 確定した方針

- 一般公開し、管理者の承認なしで登録できる。登録直後に `standard-user` となる。
- [ロール定義](../domain/role.md)にある機能をリリース対象に含める。通常コンテスト、virtual-contest、Clar、解説投稿も対象である。
- WebUI は neoShowcase、API・DB・ジャッジ基盤などは AWS に配置する方針とする。
- 提出ソースと作問ファイルは S3 に保存する。提出ソース本体は DB に保存しない。
- ジャッジ要求用・結果用のキューには SQS を採用する方針とする。
- DB は PostgreSQL 互換を優先する。提供サービスは未決である。
- メール認証にはマジックリンクを使い、パスワードは使わない。メール送信要求はキューに入れる。
- メール配送失敗への専用ハンドリング・アプリケーションによる自動再試行は設けず、利用者の再送要求で対応する。
- コンテスト得点は各問題の最高得点の合計とし、同得点時の比較方式を `contest-setter` が選ぶ。詳細は[機能要求](14-specified-requirements/02-functions/functions.md)に記す。
- 基盤を後から差し替えられる構成とする。具体的な標準スタックと費用は後で検討する。

## 文書一覧

| 章 | 内容 |
| --- | --- |
| [01 識別](01-identification/identification.md) | 対象製品・版 |
| [02 前付け](02-front-matter/front-matter.md) | 文書の状態・読み方 |
| [03 定義](03-definitions/definitions.md) | 用語 |
| [04 参照文献](04-references/references.md) | 根拠・関連資料 |
| [05 頭字語および略語](05-acronyms-and-abbreviations/acronyms-and-abbreviations.md) | 略語・判定名 |
| [06 目的](06-purpose/purpose.md) | 提供する価値 |
| [07 スコープ](07-scope/scope.md) | リリース対象と未決の境界 |
| [08 製品の位置づけ](08-product-perspective/product-perspective.md) | 構成・通信・配置 |
| [09 製品機能](09-product-functions/product-functions.md) | 機能群 |
| [10 ユーザー特性](10-user-characteristics/user-characteristics.md) | 利用者・ロール |
| [11 制限事項](11-limitations/limitations.md) | 構成上の制約 |
| [12 前提条件および依存関係](12-assumptions-and-dependencies/assumptions-and-dependencies.md) | 外部依存・資料間の差 |
| [13 要求事項の割り当て](13-apportioning-of-requirements/apportioning-of-requirements.md) | 各要素の責務 |
| [14.01 外部インターフェース](14-specified-requirements/01-external-interfaces/external-interfaces.md) | 接続境界 |
| [14.02 機能](14-specified-requirements/02-functions/functions.md) | 合意済みの機能要求 |
| [14.03 ユーザビリティ](14-specified-requirements/03-usability-requirements/usability-requirements.md) | 操作性 |
| [14.04 性能](14-specified-requirements/04-performance-requirements/performance-requirements.md) | 負荷・性能目標 |
| [14.05 論理データベース](14-specified-requirements/05-logical-database-requirements/logical-database-requirements.md) | データの責務・関係 |
| [14.06 設計上の制約](14-specified-requirements/06-design-constraints/design-constraints.md) | 配置・技術・差し替え |
| [14.07 標準への準拠](14-specified-requirements/07-standards-compliance/standards-compliance.md) | 参照規格の適用範囲 |
| [14.08 システム属性](14-specified-requirements/08-software-system-attributes/software-system-attributes.md) | 可用性・観測・運用 |
| [14.09 検証](14-specified-requirements/09-verification/verification.md) | 要求の確認方法 |
| [15 補足情報](15-supporting-information/supporting-information.md) | 未決事項の所在と保留事項 |

未決事項は各章の `TODO` を参照する。
