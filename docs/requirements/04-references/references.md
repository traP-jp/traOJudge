# 参照文献

| 資料 | 用途・扱い |
| --- | --- |
| [ロール定義](../../domain/role.md) | リリース対象の機能・操作権限の基準 |
| [API仕様](../../API/front_back.yaml) | シナリオと権限から再定義したOpenAPI 3.1.1のHTTP契約案。paths・schemas・componentsへ分割 |
| [サービス構成図](../../services/architecture-overview.puml) | 既存の構成案。API・DBの配置変更などは本要求仕様を参照 |
| [CLI問題登録API](../../API/paths/cli.yaml) | 登録開始・直接転送条件・検証後の完了を扱うHTTP契約 |
| [traP-jp/traO-Judge-judge](https://github.com/traP-jp/traO-Judge-judge) | ジャッジ定義の形式・実装参照。登録データの境界と必要情報は[問題登録・ジャッジ実行の受け渡し仕様](../14-specified-requirements/01-external-interfaces/problem-registration-contract.md)に記載 |
| [ジャッジ定義の作問側スキーマ](https://github.com/traP-jp/traO-Judge-judge/blob/517016d0df537683bcd7554c44bb48e6e5e6f14a/lib/judge_core/src/model/procedure/writer_schema.rs) | 確認コミット517016d0df537683bcd7554c44bb48e6e5e6f14aのProcedure定義。今回の登録APIの外枠や変換担当を確定するものではない |
| [問題・提出のDB設計](../../database/problem.puml) | 永続化と再ジャッジの既存設計。提出ソースのDB保存は今回の方針で変更 |
| [利用者のDB設計](../../database/user.puml) | 利用者・認証情報の既存設計。マジックリンク方式に合わせた見直しが必要 |
| [CCS Specifications 2026-01](https://ccs-specs.icpc.io/2026-01/) | コンテスト制御システムの参考仕様 |
| [CCS Scoring Data Generation](https://ccs-specs.icpc.io/2026-01/ccs_system_requirements#scoring-data-generation) | 問題ごとのペナルティ時間と、その合計の定義の参照先 |

## 資料間の優先関係

今回確定した要求は本書に記載する。既存資料と異なる点は、その資料が既に更新されたとは扱わず、追従が必要な項目として残す。今回確定していない既存資料の記述は、採用候補・設計資料として扱う。

- TODO REF-01: API仕様・構成図・DB設計を本要求仕様と整合させ、資料間の継続的な管理方法を決める。
