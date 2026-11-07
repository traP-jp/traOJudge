# traOJudge 要求仕様

対象は、traOJudge のリリース時点で満たす要件である。現在の実装の説明ではなく、リリースに向けたシステム全体の分割、責務、合意済みの要求を記述する。

共有用語は[用語集](../domain/GLOSSARY.md)に定義する。

全章に本文を記載し、未決事項は `TODO` として残す。具体的なスタック、インスタンス、費用などを未決のまま確定事項として扱わない。

全体の要件整理では、UI・AWS側のシステム・CLIの責務と受け渡しを優先する。APIサーバーとインフラはまとめて[構成ブロック](../services/aws-building-blocks.md)に分解し、接続を具体化してから実装作業を分担する。DAGの解決などのジャッジ制御はAPI側、実行環境周りはインフラ側で実装するという担当意図を維持する。

## システムの要素分割

以下は責務を整理するための論理的な分割である。要素ごとに別サービス・別プロセスを配置することまでは要求しない。

| 要素 | 責務 |
| --- | --- |
| WebUI | 公開情報の閲覧、作問、提出、コンテスト参加・運営の操作画面 |
| API | WebUI等のAPIとCLI専用APIを提供し、各機能・基盤へ処理をつなぐインターフェース |
| 利用者・認証・権限 | 利用者の登録、ログイン、認証状態、ロールに基づく操作権限 |
| 問題・作問・解説 | 問題、テストケース、judge-definition、解説、共同作業者の管理 |
| コンテスト・順位 | 通常・バーチャルコンテスト、参加、開催ルール、Clar、得点と順位 |
| 提出・ジャッジ管理 | 提出の受付、ファイル参照、DAGの解決・実行手順の具体化・実行順序の制御、結果の記録、再ジャッジ。APIサーバー担当が実装 |
| ジャッジ実行基盤 | 必要ファイルの取得、ジャッジプログラムの実行、得点・判定結果の報告 |
| データベース | 利用者、問題、コンテスト、提出・結果などの構造化データの永続化 |
| ファイル保管 | 提出ソースと作問ファイルの実体の保存・取得 |
| ジョブキュー | ジャッジ要求と結果の非同期受け渡し |
| メール送信 | マジックリンクの送信要求の受け取りと配送 |
| CLI | 問題登録などをコマンドラインから実行するクライアント。独立した担当分野とする。既存構成図のTraoPyとの関係は未決 |

配置と通信は[製品の位置づけ](08-product-perspective/product-perspective.md)、責務と要求の対応は[要求事項の割り当て](13-apportioning-of-requirements/apportioning-of-requirements.md)に記す。

CLI・Back・Judgeで共有する問題登録データと必要な契約情報は[問題登録・ジャッジ実行の受け渡し仕様](14-specified-requirements/01-external-interfaces/problem-registration-contract.md)に記す。

## 確定した方針

- 一般公開し、管理者の承認なしで登録できる。登録完了時に `standard-user` となり、そのままログイン済みになる。
- [ロール定義](../domain/role.md)にある機能をリリース対象に含める。通常コンテスト、virtual-contest、Clar、解説投稿も対象である。
- 問題登録をCLIから実行できることを必須とし、CLIを独立した担当分野として置く。
- CLI専用APIを用意する。WebUIとCLIの操作APIを共通化することは要求しない。
- CLI専用APIは利用者に紐づく開発者トークンで認証し、利用者自身がトークンを発行・失効できる。
- CLIのファイル登録は、専用APIで権限確認と署名付きURL発行を行い、CLIからS3へ直接アップロードしてAPIへ完了通知する。
- CLIは、DAG・実行環境を含むメタデータと、全テストケース・テストコード・問題得点計算ロジックを含むファイル集合を提供する。ジャッジ定義の詳細はJudgeリポジトリを参照する。
- ジャッジ定義とジャッジ実行要求を区別する。APIサーバーが固定した問題定義と提出固有入力から実行手順を具体化し、DAGの依存関係に従って実行を制御する。
- 実行環境の定義・構築・配置・運用と、BackのDB・S3・SQS接続実装はインフラ担当が担う。
- APIのジャッジ制御から実行環境へ渡す処理は、DAGの実行ノード1つずつとする。制御側が依存関係を解決し、各ノードの結果に基づいて後続を選ぶ。
- 提出コードのS3参照は保管先・オブジェクトキー・必要な版情報で渡し、Judgeの実行基盤の権限で取得する。
- 提出言語はUI・CLI・Back・Judgeで共有するサービス共通の言語IDとし、Backが受け付けたIDをそのままJudgeへ渡す。
- 問題ごとの禁止言語タグで使用可否を制限し、言語更新後も過去問題への提出を継続できるようにする。タグの判定・管理の詳細はBackのTODOとする。
- WebUIの提出ソースはAPIが発行する署名付きURLでブラウザからS3へ直接アップロード・ダウンロードする。提出者本人は自分の提出をいつでも取得でき、チーム戦では同じチームのメンバーも取得できる。通常大会・バーチャル大会の終了後は、非公開問題への提出も含め、未ログイン利用者を含む全利用者が大会内の提出情報とソースを閲覧できる。提出確定とジャッジ依頼はBackが担う。
- WebUI は neoShowcase、API・DB・ジャッジ基盤などは AWS に配置する方針とする。
- WebUIとAPIはHTTPSの同一オリジンで公開し、WebUI側のリバースプロキシが `/api/*` をAWSのAPIへ転送する。
- 提出ソースと作問ファイルは S3 に保存する。提出ソース本体は DB に保存しない。
- ジャッジ要求用・結果用のキューには SQS を採用する方針とする。
- 実行基盤はDBを直接読み書きせず、SQSで要求・結果を受け渡し、必要ファイルをS3から取得する。ジャッジ制御と結果のDB反映はAPIサーバーが担う。
- ジャッジ要求の作成時に、使用する問題・ジャッジ定義・ファイルの版を固定する。更新版を使う場合は別の実行要求とする。
- Backが要求ごとに実行IDを発行し、Judgeが結果に同じIDを付ける。再ジャッジは別ID、同じ要求の再配信は同じIDとする。
- 同じ実行IDの同じ結果が重複して届いても、Backが一回分として反映し、得点や結果を二重計上しない。
- JudgeはDAGのノード単位で結果を返し、Backが作問者からCLIで登録された情報に基づいて解釈する。
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

実装着手に向けた現状調査と作業票は[PoC 実装に向けた棚卸し](poc-work-breakdown.md)、AWS側の分解案は[構成ブロック](../services/aws-building-blocks.md)に記す。構成候補・PoCの提案と、上記の合意済みリリース要求は区別する。

ユーザーからのmutationを時系列につなぎ、独立した作業へ切り出す案は[実装シナリオ](mutation-scenarios.md)に記す。読み取りAPIの全体とクラスタ内部の通信形式は別に具体化する。提出後に完了ケース数を表示するなど、途中で利用者に見せたいことから必要になる条件は、[進捗表示の制約](submission-progress-constraints.md)で先に整理する。

各操作について、誰が誰に・どの問題や大会に何をしてよいかは[操作ごとの許可条件](permission-operations.md)を参照する。役割の付与・剥奪は、役割ごとに分けたシナリオへつなぐ。
