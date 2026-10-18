# 参照文献

| 資料 | 用途・扱い |
| --- | --- |
| [ロール定義](../../domain/role.md) | リリース対象の機能・操作権限の基準 |
| [API仕様](../../API/front_back.yaml) | 既存の機能・外部インターフェースの資料。今回の合意による更新が必要 |
| [サービス構成図](../../services/architecture-overview.puml) | 既存の構成案。API・DBの配置変更などは本要求仕様を参照 |
| [問題更新プロトコル](../../API/problem_update_protocol.puml) | 更新開始・署名付きアップロード・完了通知の設計 |
| [問題・提出のDB設計](../../database/problem.puml) | 永続化と再ジャッジの既存設計。提出ソースのDB保存は今回の方針で変更 |
| [利用者のDB設計](../../database/user.puml) | 利用者・認証情報の既存設計。マジックリンク方式に合わせた見直しが必要 |
| [CCS Specifications 2026-01](https://ccs-specs.icpc.io/2026-01/) | コンテスト制御システムの参考仕様 |
| [CCS Scoring Data Generation](https://ccs-specs.icpc.io/2026-01/ccs_system_requirements#scoring-data-generation) | 問題ごとのペナルティ時間と、その合計の定義の参照先 |

## 資料間の優先関係

今回確定した要求は本書に記載する。既存資料と異なる点は、その資料が既に更新されたとは扱わず、追従が必要な項目として残す。今回確定していない既存資料の記述は、採用候補・設計資料として扱う。

- TODO REF-01: API仕様・構成図・DB設計を本要求仕様と整合させ、資料間の継続的な管理方法を決める。
