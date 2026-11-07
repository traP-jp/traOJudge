# 頭字語および略語

| 略語 | 意味 |
| --- | --- |
| API | Application Programming Interface |
| AWS | Amazon Web Services |
| DB | Database |
| UI | User Interface |
| CLI | Command Line Interface。問題登録などをコマンドラインから行うクライアント |
| DAG | Directed Acyclic Graph。ジャッジ処理の依存関係を表す有向非巡回グラフ |
| S3 | Amazon Simple Storage Service。ファイル保管の採用方針 |
| SQS | Amazon Simple Queue Service。ジャッジ要求・結果キューの採用方針 |
| CCS | Contest Control System |
| ICPC | International Collegiate Programming Contest。本書ではタイブレーク方式の名称にも用いる |
| AC | Accepted。ジャッジの判定名 |
| CE | Compilation Error。ペナルティ回数から除外する判定 |
| IE | Internal Error。サービス側エラーを表す判定 |
| WE | Writer Error。作問側エラーを表す判定 |
| TODO | 未決事項。仕様・数値・方式などを後から決める必要があることを示す |

- TODO ACR-01: 判定名の全一覧と API・DB・ジャッジ実装間の対応を統一する。既存APIとDBでは WE の有無が一致していない。
