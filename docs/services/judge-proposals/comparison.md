# 二つの独立提案の比較

[固定ワークフローとSQS callback](sqs-callback.md)は独自のフロー記述言語を解釈し、[作問者ASLとAWS Batch](native-asl-batch.md)はAmazon States Language（ASL）でフローを定義する。どちらも未実装の提案である。各提案の設計とAWSの公開仕様に基づき、実装責任を比較する。

| 比較項目 | [固定ワークフローとSQS callback](sqs-callback.md) | [作問者ASLとAWS Batch](native-asl-batch.md) |
| --- | --- | --- |
| フローの登録 | 独自言語の検証、解釈、依存解決を実装する。Step Functionsの定義は固定する。 | 登録APIがASLを検証し、問題別の定義と[不変のバージョン](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-state-machine-version.html)を公開する。呼び出し先や資源上限の制限は独自実装になる。 |
| ワーカーへの依頼 | [SQS callback連携](https://docs.aws.amazon.com/step-functions/latest/dg/connect-sqs.html)で完了を待つ。[SQS Standardは重複配送し得る](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/standard-queues-at-least-once-delivery.html)ため、実行権の期限と世代を管理し、古い結果を拒否する。 | [Batchの同期連携](https://docs.aws.amazon.com/step-functions/latest/dg/connect-batch.html)で完了を待つ。配置と[設定回数までの再試行](https://docs.aws.amazon.com/batch/latest/userguide/job_retries.html)をBatchに任せ、成功した試行の成果物をアプリが検証する。 |
| 期限と回収 | callbackと実行権に期限を設け、結果保存後の応答消失には保存済み結果を再利用する。起動漏れやワークフロー停止は定期照合で回収する。 | [実行可能状態での滞留](https://docs.aws.amazon.com/batch/latest/userguide/job_states.html)と、[起動中を含まない実行タイムアウト](https://docs.aws.amazon.com/batch/latest/userguide/job_timeouts.html)を補う絶対期限が必要になる。定期照合で未起動や未終端を回収する。 |
| 最終失敗 | DynamoDBの失敗記録とSQSへの送信待ちを同時に保存し、未送信分を再送する。ノード配送用の失敗キューも設ける。 | DynamoDBの`DEAD_LETTER`台帳に理由と再判定用の参照を残す。独立提案ではSQSを使わない。 |
| EC2の最大5台 | Auto Scalingを使う。[再配置中の一時的な上限超過](https://docs.aws.amazon.com/autoscaling/ec2/userguide/auto-scaling-benefits.html)を含めた制御が必要になる。 | 単一機種と`BEST_FIT`を候補とする。[割当方式によっては最大1台分超過する](https://docs.aws.amazon.com/batch/latest/APIReference/API_ComputeResource.html)ため、方式の選択と計算環境交換時の台数管理が必要になる。 |

両案とも、[DynamoDBの条件付きトランザクション](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html)を使って最終判定を一度だけ確定する設計であり、物理的な再実行は許容する。[対応EC2のネスト仮想化](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/amazon-ec2-nested-virtualization.html)はKVMを使う前提を満たすが、Firecrackerとの統合、隔離、採点の公平性は実機検証が残る。

同条件で両案を実装した比較データはなく、費用、速度、信頼性の優劣は未確認である。[公平性実験](../fairness-experiment.md)と[PostgreSQL pool実験](../postgres-pool-experiment.md)は別構成の測定であり、この二案の性能根拠にはならない。

[現行構成](../aws-judge-orchestration.md)は作問者ASLとAWS Batchを基に、最終失敗のSQS配送を追加している。
