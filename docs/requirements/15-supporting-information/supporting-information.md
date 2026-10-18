# 補足情報

## 未決事項の所在

未決部分は各章の `TODO` として管理する。要求の本文とTODOを併記し、空の章や暗黙の前提で補わない。

| 領域 | 参照先 |
| --- | --- |
| 文書の対象版・管理責任者 | [識別](../01-identification/identification.md)、[前付け](../02-front-matter/front-matter.md) |
| 用語・問題種別・リリース境界 | [定義](../03-definitions/definitions.md)、[スコープ](../07-scope/scope.md) |
| 要素間の責任分担・接続 | [構成](../08-product-perspective/product-perspective.md)、[割り当て](../13-apportioning-of-requirements/apportioning-of-requirements.md)、[外部IF](../14-specified-requirements/01-external-interfaces/external-interfaces.md) |
| CLIの範囲と分野間契約 | [スコープ](../07-scope/scope.md)、[割り当て](../13-apportioning-of-requirements/apportioning-of-requirements.md)、[機能](../14-specified-requirements/02-functions/functions.md) |
| 受け渡し仕様の参照先と契約に必要な情報 | [問題登録・ジャッジ実行の受け渡し仕様](../14-specified-requirements/01-external-interfaces/problem-registration-contract.md) |
| 認証・権限・各機能 | [機能](../14-specified-requirements/02-functions/functions.md) |
| データと保存期間 | [論理DB](../14-specified-requirements/05-logical-database-requirements/logical-database-requirements.md) |
| 差し替え・スタック・実行環境 | [設計上の制約](../14-specified-requirements/06-design-constraints/design-constraints.md) |
| 操作性・性能・運用・検証 | 14章の各該当項目 |
| 既存資料との整合 | [参照文献](../04-references/references.md)、[依存関係](../12-assumptions-and-dependencies/assumptions-and-dependencies.md) |

## 確定扱いにしない事項

- single-player・two-playerのリリース採用範囲。
- CLIの問題登録以外の操作範囲と、既存のTraoPyを実装として採用するかどうか。
- 問題登録時の正規化・登録済み形式への変換を担当する分野と、具体的な受け渡し形式。提出後の実行手順の具体化はJudgeの責務として確定している。
- APIの実行サービス・インスタンス。ECS/Fargateを採用済みとは扱わない。
- DBの具体的な提供サービス。RDSを採用済みとは扱わない。
- 月額費用の上限や、具体的な性能・可用性の数値。
- 認証をAPIへ集約してneoShowcaseから独立させるかどうか。
- 差し替え時にドメインロジック・公開APIの変更を完全に禁止するかどうか。

## 補足

全章への記入は、すべての詳細が確定したことを意味しない。現時点の成果は、システム全体の要素分割と合意事項を明文化し、未決部分をTODOとして特定した要求仕様である。
