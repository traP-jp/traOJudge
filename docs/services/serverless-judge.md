# ジャッジ基盤：今回の実装で何が分かったか

**1 VMで同時に1ノードを実行し、最大5台まで増やす構成**。非ACを判定jobの失敗に変換し、既存Rust Engineが同じpipelineの新規dispatchを止める。[木構造の実演動画・38秒](demos/tree-fail-fast.mp4)と[説明・証拠](tree-demo.md)を追加した。一方、**必要時刻までの増設、通常10秒以内、請求総額の制限は達成していない**。

## 質問への回答

実験A/Bは`13747d5`時点の記録。Aは受信待ちと退役の競合修正前、Bはその修正後。どちらも今回の非AC→job_failed変換より前である。**実験Cが修正後の木構造デモ**で、CでA/Bの全項目を再実験したわけではない。

| 質問 | 今回の回答と証拠 | 残っていること |
| --- | --- | --- |
| 木構造のtest-caseでfail-fastできるか | **Cの実AWS動画で確認。** 非ACをjob_failedとして渡すと、Engineが同じpipelineの新規dispatchを止める。発行済みrunは終了するが、そのcheckや別枝の後続は発行されない。 | 製品の問題登録APIとの接続。Aの独自な子孫SKIPは削除済み。 |
| 将来の需要に間に合うよう増設できるか | **台数の先読みは実装、時刻に間に合う実証は未達。** 240秒内の既知の枝から必要台数を計算。Aでは5台readyの初観測が広い層の開始より0.900秒後だった。 | 起動時間を含む時刻別の計画、供給遅延の改善。 |
| 連続提出でVMを再利用するか。cleanupは | **再利用する。Aで確認。** 実行ごとにsandboxと作業領域を削除・検証してから次へ進む。同じVMでの2提出は4.756秒／10.830秒。 | cleanup失敗時のVM自動交換。隔離からの脱出全般を証明した試験ではない。 |
| 再利用しない方式は実用的か | **未測定。現在はその方式を採用していない。** 退役後の代替VM待ちでは、Bの結果保存まで約284〜286秒かかった。 | 毎ノード新規VM方式との比較。通常10秒目標への適合。 |
| 待ちjobがあるとき、どうscale-inするか | 待ち・実行中・先読み需要を見て余剰VMを退役させる。ただし**既に退役中のVMは新規提出が来ても再開しない**。Aでは87.110秒、Bでは約284〜286秒待って完了した。 | 継続的な待ち行列がある状態での十分な検証、待ち時間の上限。 |
| 実行中のVMを回収しない保証は | 通常のASG縮小には保護をかけ、受信待ち・実行・結果配送・cleanupが終わるまで解除しない。**Bで退役後に返った予約済み受信からの実行を確認。** | EC2障害や外部からの強制停止までは防げない。 |
| 回収・喪失後にリトライできるか | **Aで実際にpoweroffして確認。** 同じrunが実行中停止から228.783秒、結果保存後停止から186.684秒でACに復旧した。 | 無限の成功保証やexactly-onceではない。修正後ソースでの喪失試験一式は未実施。 |
| DLQはどうなるか | **NodeDLQはAで確認。** 5回受信しても処理できない要求を隔離保存し、基盤障害として終端化。明示再判定は新runでAC。 | ControlDLQと受付streamの回復処理。BでDLQが空だったことは障害回復試験の代わりにならない。 |
| 並列上限と破産対策は | **通常のpool上限は5台。** Aの保存済み実行区間で最大5並行、Bで最大3並行。EC2＋IPv4の概算は5台で約$0.59/時。 | 総額の予算停止、受付量・保存量・保持期間・稼働期限の制限。5台上限だけでは総額を抑え切れない。 |

詳細と測定条件は[質問別の実験レポート](scaling-experiment.md)、設計を変更する議論は[独立したpool policy](judge-pool-policy.md)へ。

## デモ・証拠を見る

**現行の木構造デモは実験Cの動画。** 5台warmから2件を提出し、非ACと全ACのschedulerの動作を等速で録画した。既知の将来需要への増設が間に合うことを示す動画ではない。再実行用コードも残し、録画用の専用stackは使用後に削除した。

| 見たいもの | リンク | 読めること・見られること |
| --- | --- | --- |
| 今回の木構造デモ（C） | [動画・38秒](demos/tree-fail-fast.mp4)・[見どころ](tree-demo.md)・[検証JSON](demos/tree-fail-fast-evidence.json) | 非AC→job_failed→新規dispatch停止。発行済みrunの終了と、全AC時のsummarize。 |
| 今回の実験の結論 | [実験レポート](scaling-experiment.md) | 上の各回答の実測値・限界。 |
| 実験Aの記録 | [観測JSON](scaling-experiment.json)・[runner](../../infra/experiments/run_scaling.py) | 木の打切り、5並行、再利用、強制停止、NodeDLQ、退役中の到着。修正前ソースの結果。 |
| 実験Bの記録 | [観測JSON](drain-experiment.json)・[runner](../../infra/experiments/run_drain.py) | 受信待ちと退役の競合。5提出・30ノードが完了、受信回数は各1回。 |
| 現行の構成図 | [PNG](diagrams/judge-pool.png)・[HTML](diagrams/judge-pool.html) | 構成の説明図。実際の動作の録画ではない。 |
| 以前の表示デモ | [動画・39秒](history/2026-11-02-prototype/demos/judge-flow.mp4)・[静止画](history/2026-11-02-prototype/demos/judge-flow-poster.png)・[観測記録](history/2026-11-02-prototype/demos/judge-flow-evidence.json) | **旧1 VM・2同時実行**のbuild→並行run/check→summarizeと進捗・結果表示。通信はJSON polling。現行の1 VM・同時1実行、複数VMへの増設を示す動画ではない。 |

## 用語と設計の入口

- **提出／run／ノード**：1提出の判定1回がrun。runの中のbuild、各caseのrun/check、summarizeがそれぞれ実行ノードになる。1 VM・同時1実行は、このノードを同時に2つ載せないという意味。
- **再利用／退役／破棄**：起動済みで退役前のVMはcleanup後に再利用する。退役を決めたVMは新しい受信を止め、処理を終えて破棄する。破棄したVMを復活させる仕組みではない。
- **台数の計算／実際の増減**：自作controllerが必要台数を計算してmetricを発行し、CloudFormationに定義したAWSのalarmとscaling policyがASGを増減する。計算部分までAWSのmanaged schedulerになったわけではない。

設計の正本は[pool.json](../../infra/policy/pool.json)と[AWS非依存の判定関数](../../infra/runtime/policy.py)。[policy文書](judge-pool-policy.md)に設定値と安全条件、[関心事一覧](serverless-judge-concerns.md)に未決事項をまとめた。[過去の試作](history/2026-11-02-prototype/README.md)は現行設計から分離している。
