# 作問者が登録する判定フローをAWSで実行する構成

Step Functions Standardがフローを進め、AWS BatchがEC2へ処理を配置する。提出の受理順は保持するが、実行順と完了順は制限しない。最終判定の確定は一度だけとし、基盤障害で判定できなければ失敗を確定してSQSへ配送する。プログラムの再実行はあり得る。

これは保証条件の設計であり、実装・検証済みを意味しない。終端への収束には、AWSの復旧、定期照合の継続、必要な権限とデータの保持が必要になる。

![判定フローの実行構成](diagrams/aws-judge-orchestration.png)

[HTML版](diagrams/aws-judge-orchestration.html)・[暗色版](diagrams/aws-judge-orchestration-dark.png)

## フローをデータ登録し、問題版へ固定する

作問者はAmazon States Language（ASL）の定義と、採点プログラム、テスト生成器、テストなどの問題一式をAPIで登録する。登録処理が定義を検証し、[`CreateStateMachine`](https://docs.aws.amazon.com/step-functions/latest/apireference/API_CreateStateMachine.html)、`UpdateStateMachine`、`PublishStateMachineVersion`で公開する。問題ごとのリポジトリ変更やインフラ定義のデプロイは不要である。

問題版には[不変のバージョンARN](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-state-machine-version.html)と問題一式のハッシュを保存し、提出をその版へ固定する。フロー定義と、`StartExecution.input`で渡す実行ごとの入力は分ける。

作問者は`Choice`、`Parallel`、`Map`と、制約内の`Retry`、`Catch`を定義できる。`Task`は共通の実行フローに限定し、固定のBatchキューとジョブ定義を使う。IAMロール、任意のAWS API、ホストのコマンド、ネットワーク設定は選ばせない。提出ID、再判定世代`generation`、期限も上書きさせない。

[構文検証](https://docs.aws.amazon.com/step-functions/latest/apireference/API_ValidateStateMachineDefinition.html)に加え、参照先、定義サイズ、状態数、時間、並列数、反復量を制限する。共通TaskとIAMでも制約を強制する。登録数と実行数のクォータを監視し、未使用版は提出からの参照と保存方針を確認して整理する。

## 受付と起動待ちを同時に保存する

入力と成果物はS3へ保存し、不変の版または内容のハッシュで参照する。受付は、範囲ごとのカウンターを条件付きで更新し、提出、冪等キー、問題版、入力参照、絶対期限、`START_PENDING`の起動待ちを[同じDynamoDBトランザクション](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html)で確定する。同時到着の順序は確定順とし、UIは`submissionSeq`で並べる。同じキーと内容の再送には既存の提出と順番を返す。

確定前にAPIが停止した場合は再送し、確定後なら保存した起動待ちをStreamsの再送か定期照合で回収する。通知だけに依存しない。

起動処理は判定IDとgenerationから決めた実行名と、保存済みの同じ入力を使う。成功応答を失ったら既存の実行を確認し、終了済みなら`ExecutionAlreadyExists`を処理する。[名前は90日後に再利用できる](https://docs.aws.amazon.com/step-functions/latest/apireference/API_StartExecution.html)ため、重複防止の正本は受付台帳に置く。

## Batchが配置とワーカーの再試行を担う

固定の監督用フローが問題版の子フローを待ち、結果形式を検証する。子フローの共通Taskは[Batchの同期連携](https://docs.aws.amazon.com/step-functions/latest/dg/connect-batch.html)でジョブを投入する。依存解決と実行順序はStep Functions、[実行待ちと資源への配置](https://docs.aws.amazon.com/batch/latest/userguide/job_states.html)はBatch、ホストでの開始はECSエージェントが担う。独自のキュー監視、実行権の更新、リーダー選出は設けない。

ワーカーやEC2が停止したら、Batchの有限回の[`RetryStrategy`](https://docs.aws.amazon.com/batch/latest/userguide/job_retries.html)で再試行し、上限ならフローの`Catch`か最終失敗へ進む。ジョブとフローそれぞれの再試行上限と期限を定め、再試行の積み重ねを制限する。成果物はジョブIDと試行ごとに保存し、成功した試行の成果物一覧だけを共通Taskが検証する。Batchの成功応答自体は判定結果ではない。

## 最終判定か失敗を一度だけ確定する

状態は`START_PENDING`、`RUNNING`、`FINALIZING`、`JUDGED`、`DLQ_PENDING`、`DLQ`を持つ。`(submissionId, generation)`の終端結果は未確定の場合だけDynamoDBトランザクションで作る。成功、期限超過、失敗が競合しても上書きせず、応答消失後の再実行には既存結果を返す。

WA、CE、TLE、RE、MLEは正常なジョブ出力として返す判定結果である。実行管理プログラムの異常、VM起動不能、成果物不備は再試行や失敗の対象とする。子ノードの失敗はフローで代替処理でき、Batchの`FAILED`だけでは提出全体を失敗にしない。

監督用フローが共通の条件付き確定処理を呼ぶ。異常終了時は定期照合が同じ処理で回収する。ワーカーに終端更新権限は与えず、古い試行にも確定結果を変えさせない。

最終失敗では、理由、入力、問題版、実行とジョブの参照、およびSQS失敗キューへの配送待ちを同じトランザクションで保存する。送信成功を確認するまで`DLQ_PENDING`と配送待ちを残して再送し、確認後に`DLQ`とする。応答消失でメッセージは重複し得るが、失敗の確定は1件である。SQSの保持期間後も失敗台帳と入力参照を保持する。

## 期限と定期照合で停止を回収する

受付時の絶対期限は延長しない。Batchの`RUNNABLE`は無期限に続き得て、[実行timeoutに`STARTING`は含まれない](https://docs.aws.amazon.com/batch/latest/userguide/job_timeouts.html)。監督用フローの期限と独立した定期照合で、未起動、未終端、期限超過、SQS未配送を回収する。照合処理はノード配布や依存解決を行わない。

[Streamsの保持は24時間](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/HowItWorks.CoreComponents.html)で、[Step Functionsの通知は配送を保証しない](https://docs.aws.amazon.com/step-functions/latest/dg/eventbridge-integration.html)。フロー自身の`Catch`では時間切れや強制停止を必ず回収できない。`FAILED`、`TIMED_OUT`、`ABORTED`の実行は台帳と照合し、保存済み結果を回収するか失敗を確定する。

Lambdaなどのアプリや全ワーカーが再起動しても、保存状態から再開する。短い処理の再試行は副作用を冪等にし、メモリ復元に依存しない。Step Functionsの制御面やBatchの配置処理の一時障害はAWS復旧後に継続し、期限超過は照合で回収する。`.sync`の通知が遅れた場合も照会でジョブの終端を確認する。

### 終端後も計算資源の停止を確認する

[同期連携の中断時の取消しは成功を保証しない](https://docs.aws.amazon.com/step-functions/latest/dg/connect-to-resource.html)。既知の子フローとジョブを取り消し、必要なら終了させ、後片付けの記録で停止確認まで追跡する。

投入時は提出ID、generation、ノード呼び出しの識別子を`jobName`と`tags`へ固定する。`jobName`は冪等キーではない。`SubmitJob`の応答消失でIDが残らなくても、対象キューを投入時刻と名前でページごとに調べ、`DescribeJobs`の`tags`と照合する。再送による重複も終端後まで追跡する。

実行管理プログラムはゲスト開始前にgeneration、終端状態、絶対期限を台帳で確認する。期限切れ、確定済み、確認不能なら開始しない。ゲストごとの実時間上限も強制する。

## 敵対的コードを隔離する

信頼する実行管理プログラムがFirecrackerなどのmicroVMでコンパイラ、提出、採点プログラムを動かし、ゲストにNIC、AWS資格情報、ホストのファイルシステムを渡さない。VMとケースごとのCPU、メモリ、プロセス数、実時間、出力、ファイルの上限、jailer、seccomp、カーネル更新は実行管理プログラムの責務とする。[ECSの権限境界](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task-iam-roles.html)と[Firecrackerの運用要件](https://github.com/firecracker-microvm/firecracker/blob/main/docs/prod-host-setup.md)に従う。

VMはECSタスクの子cgroupに置き、タスク停止後にVMの管理プロセスを残さない。[対応EC2のネスト仮想化](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/amazon-ec2-nested-virtualization.html)を利用できるため、ベアメタルは必須としない。

## EC2を最大5台に抑える条件

候補は単一のインスタンスタイプ、On-Demandの`BEST_FIT`とし、`maxvCpus`を5台分に設定する。[`BEST_FIT_PROGRESSIVE`などは最大1台分を超過し得る](https://docs.aws.amazon.com/batch/latest/APIReference/API_ComputeResource.html)ため、この値だけでは5台を保証しない。`BEST_FIT`の基盤更新の制約と、計算環境の交換時の新旧合計も管理する。CPU登録数とジョブ予約量を合わせ、ホストごとの同時実行数を定める。

`minvCpus=0`なら無負荷時のEC2費を抑えられる。[Batchの追加料金はない](https://aws.amazon.com/batch/pricing/)が、保存と照合の費用は残る。常時稼働のDB、NAT、ALB、有料エンドポイントは必須にしない。公平性と、EC2停止状態から10秒以内に結果を返すことは未検証であり保証しない。

## 通知欠落後も進捗を再取得する

実行管理プログラムがDynamoDBへ進捗を保存し、Streamsの配信処理が[AppSync Events](https://docs.aws.amazon.com/appsync/latest/eventapi/event-api-concepts.html)へ通知する。提出ID、再判定世代、ノードID、試行、更新番号で重複と古い進捗を除く。

初回と再接続時はスナップショットを取得する。購読中のイベントを一時保存し、スナップショットより新しい更新だけを適用する。配信停止や通知欠落は再取得で修復する。接続中も非終端の提出は低頻度で再取得し、最後の通知が欠落しても終端へ追いつく。
