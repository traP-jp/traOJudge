# ジャッジ実行の公平性とzeroからの応答時間：実測

**固定したVMで同時1実行にするだけでは、判定時間の公平性を確定できなかった。**
今回のgVisor内の`getrusage`には大きな量子化と変動があり、同じEC2上のnative対照実験と異なる。
Lambda MicroVMsはCPU中心の計算が安定し、計算資源停止からの提出も10秒以内だったが、
メモリ中心の計算には変動が残った。いずれも、そのまま本番採用を確定する結果ではない。

2026-11-02、東京リージョンで実測した。`develop-hayatroid`の`d2400cb`を起点とする
`codex-judge-fairness`の独立した最小再現であり、既存のserverless試作を採用したものではない。

- [比較構成図](diagrams/fairness-comparison.png)・[暗色版](diagrams/fairness-comparison-dark.png)・[HTML](diagrams/fairness-comparison.html)
- [CPU時間とwall timeの分布図](diagrams/fairness-distribution.png)
- [集計JSON](fairness-summary.json)・[環境と固定値](fairness-environment.json)・[撤去確認](fairness-teardown.json)
- 実装：[fixture・計測器・collector](../../infra/experiments/fairness/)、[MicroVMのCDK](../../infra/bin/fairness.ts)、[EC2のCDK](../../infra/bin/fairness-ec2.ts)、[提出キューのCDK](../../infra/bin/fairness-queue.ts)

## 比較条件

| 構成 | 条件 |
| --- | --- |
| Lambda MicroVMs | 3 VM。ARM64、baseline 2 GiB、guestから4 CPUが見える。提出プロセスをCPU 0へ固定し、seccompで親子のaffinity変更を禁止。runtimeのAWS roleなし。egressはoutbound許可のないisolated VPC connectorへ。 |
| EC2＋gVisor | 2台の`c7g.large`。AWS APIの仕様値は2 cores、各core 1 thread。agentはCPU 0、containerは`cpuset=1`、同時1 container。network none、read-only root、512 MiB、64 PIDs、bounded tmpfs。gVisorは`release-20261026.0`のARM64配布物をSHA-512固定。 |
| EC2 native対照 | 同じ2台・CPU slot・C fixtureを`runc`で測る。gVisorの計測値とhardware実行の変動を切り分ける対照であり、本番の隔離方式として推奨するものではない。 |

全構成でARM64、GCC `14.2.0-19`、Python `3.13.18`。
compiler flagsは`-O2 -std=c17 -fno-tree-vectorize -march=armv8-a`。
CPU識別フィールドは全構成でimplementer `0x41`、part `0xd40`などが一致した。
事前ビルドfixtureのSHA-256も全構成で一致した。これは物理hostの共有状況やメモリ帯域の保証ではない。
実行時コンパイルの結果ファイル自体のhashは採取していない。
base imageとgVisorはdigest／hash固定だが、apt packageを将来の再buildまで固定するlockはない。
再現時は採取したcompiler versionとprebuilt binary hashを照合し、本番用にはtoolchain artifact自体の固定が必要。

fixtureはCPUの整数演算、32 MiB配列へのアクセス、2 childへの計算分割の3種類。
child版は各childの計算回数を半分にして合計の演算量をCPU版に揃えた。
親はchildをwaitし、計測器もsubreaperとして残った子孫をkill・waitする。
報告するCPU時間はreapしたプロセスツリーのuser＋system時間の差分。
wall time、compile時間、container時間、clientから結果を観測する時間は別々に保存した。

短いfixtureは各VM／各条件20回、長いfixtureは各VM／各条件10回。
MicroVMは各phase 60／30 sample、EC2は40／20 sampleとなる。
sequentialの後に別VMへのparallel提出を行った。phase順は固定で、ランダム化や長期試験はしていない。
外れ値は除外していない。P95はnearest rank、CVは標準偏差／平均である。
同じVM内の反復なので、sample数を独立した物理host数と解釈しない。

## CPUの実行性能と計測器の変動を分ける

短いfixtureのparallel条件は以下だった。表は判定に使う**取得CPU時間**の分布であり、
そのままhardware性能の分布を示すものではない。

| 構成 | 整数演算：中央値 / P95 | 同CV | メモリ：中央値 / P95 | 同CV |
| --- | --- | --- | --- | --- |
| MicroVM | 117.132 / 117.237 ms | 0.055% | 290.206 / 336.458 ms | 8.659% |
| EC2＋gVisor | 100 / 120 ms | 11.599% | 450 / 490 ms | 6.120% |
| EC2 native対照 | 117.722 / 117.807 ms | 0.044% | 499.832 / 509.114 ms | 1.293% |

gVisorでは取得CPU時間が10 ms刻みだった。同じ整数演算のwall timeは中央値144.165 ms、CV 1.636%で、
報告CPU時間のCV 11.599%より小さい。**「EC2の物理CPU性能が11.6%揺れた」とは結論できない。**
同じEC2のnative対照では取得CPU時間は117.722 ms付近に集中し、観測値の粒度も異なる。
この計測器をgVisor内にそのまま置いて厳密なTLEへ使う案は採用できない。
10 msの丸めだけですべての差を説明できたわけではなく、accountingの詳細な原因までは未確定である。

MicroVMのメモリfixtureはwall timeもCV 8.570%となった。
HTTPやキューの揺れを除いても、実行中の変動が残っている。
同じVM内でも観測され、CPU識別値とbinaryを揃えるだけでは抑え切れていない。
原因を物理hostの競合、cache、allocator、サービスの資源管理のいずれかに断定するデータは取っていない。

短いcaseだけの現象か確認するため、約0.7秒のchild計算と約1〜2秒のメモリ計算でも追試した。

| 構成 | 長い計算：中央値 / P95 | 同CV | 長いメモリ：中央値 / P95 | 同CV |
| --- | --- | --- | --- | --- |
| MicroVM | 695.284 / 695.478 ms | 0.013% | 1,078.006 / 1,200.161 ms | 5.269% |
| EC2＋gVisor | 640 / 710 ms | 5.654% | 1,705 / 1,800 ms | 4.616% |
| EC2 native対照 | 697.588 / 697.865 ms | 0.026% | 1,888.135 / 1,937.573 ms | 3.347% |

native対照の短い測定は事前compile済みfixture、主たるgVisor測定はjob冒頭でcompileした。
各caseのCPU計測区間にcompileは含めないが、cache状態などの条件差は残る。
長い追試は全構成で事前compile済みを使い、ここを揃えた。

## TLE境界

各構成の無制限baseline中央値の0.95倍・1.00倍・1.05倍をCPU上限にした。
したがって構成間で絶対秒数が同じ試験ではない。
baselineと完了後の値は`getrusage`差分、実行中の監視は同じprocess groupの
`/proc/*/stat`のuser／system／wait済みchildのtick合計を約3 msごとに読む実装である。
2つのAPIを組み合わせており、反転には両者の粒度・整合性も影響し得る。
gVisor一般のTLE精度の限界を証明したものではなく、この計測・監視方式の不適合を確認した。

| 条件 | MicroVM | EC2＋gVisor |
| --- | --- | --- |
| 短いcase、0.95倍、parallel | TLE 60 / 60 | TLE 33 / 40、OK 7 / 40 |
| 短いcase、1.00倍、parallel | TLE 43 / 60、OK 17 / 60 | TLE 30 / 40、OK 10 / 40 |
| 短いcase、1.05倍、parallel | OK 60 / 60 | TLE 24 / 40、OK 16 / 40 |
| 長いcase、1.05倍、parallel | OK 30 / 30 | TLE 19 / 20、OK 1 / 20 |

長いcaseのEC2 native対照は、0.95倍でTLE 20 / 20、1.05倍でOK 20 / 20だった。
MicroVMも1.00倍の境界では反転するので、ぶれゼロを証明したものではない。
gVisorのbaseline自体が粗いため「真の必要CPUに5%の余裕があるのに誤TLE」とは言えない。
同じコード・同じ試験閾値で判定が混在し、そのbaselineから安定した余裕を作れていないという結果である。

## 提出から結果までとzero

| 試験 | 実測 | 解釈 |
| --- | --- | --- |
| MicroVMの新規Run APIからcompile＋最初の結果 | 3件：5.584 / 5.693 / 6.432秒 | CFN起動処理内のcomponent測定。SQS待ちとowner Lambdaのinitは含めない。提出全体のP95とは扱わない。 |
| 0 RUNNING VMからSQS→Lambda→resume→compile→実行→S3結果観測 | 30件、中央値2.028秒、P95 3.344秒、最大4.798秒。30 / 30が10秒以内 | 毎回3 VMすべてSUSPENDEDを確認してから送信。1 slotへの逐次到着、短いC fixture 1 case。最初のworker Lambda呼出しも含む。 |
| warm EC2＋gVisor、SQS→compile→実行→S3結果観測 | 20件、P95 3.442秒、最大3.457秒 | 受付APIやDBは含めていない。 |
| EC2を実際に0台へ落としてから同じ提出 | 104.779秒、結果OK | ASG所属0・該当ASGの非terminated instance 0を確認。SQS受付直後に手動で最速wake。packages／imageを起動時に準備する、このAMI構成の結果。prepared AMI一般の限界とは扱わない。 |

**MicroVMのzeroは計算資源0であり、3つのsuspended VMとsnapshotが残っている。**
完全破棄した0 VMからの新規作成を含むSQS全体の30件試験ではない。
snapshot等の保存費用は残る。請求額や総費用の優劣は測っておらず、「平時の費用が完全に0」も証明していない。
10秒の結果はこのfixture・逐次到着条件に限られ、多言語、重いcompile、全ケース、burst時の保証ではない。

## 障害時の結果とDLQ

1提出を1メッセージとして、既存のCFN所有MicroVMをresumeするworkerを使った。
FIFOの1 message groupで同じVMへの重複配置を防ぎ、成功時は結果をS3へ条件付き保存してからACKする。
runtimeは任意のVM作成権限を持たず、guestにもAWS roleを渡さない。

- **結果保存前の基盤例外**：初回を明示的に失敗させ、受信2回目で結果OK。resultのversionは1。
- **保存後・ACK前の例外**：再配信で保存済み結果を認識し、受信2回目でACK修復。resultのversionは1。
- **毎回失敗する基盤要求**：redrive上限3回の設定で、91.267秒後に元要求がDLQへ到達。原文を記録してからACKした。
- **提出プログラムのabort**はRE、CPU上限はTLE、wall監視打切りはWATCHDOGとして分離した。

wall監視打切りという事実だけでは、プログラムのハングと基盤の過負荷を自動判別できない。
本番の再試行・IE化には別の判断条件が必要。
実VM喪失、全ての制御面障害、受付からの永久保持、未配送要求の再走査、製品APIへの終端通知は未検証である。
DLQの14日保持も永久的な台帳の代わりにはならない。

## 選定への判断

- **同じCPU型と同時1実行は出発点になるが、十分条件ではない。** 物理性能と計測APIの精度を分けて検査する必要がある。
  物理CPUの専有が必須という必要性までは、この試験では証明していない。
- **MicroVMはzeroと低レイテンシを両立する候補として残る。** CPU中心の計算は安定したが、メモリ中心の再現性には課題が残る。
- **今回のgVisor内getrusageを、そのまま判定時間の正本にする案は採用しない。** host側を含むaccountingの設計を再検討する。
- **native対照を本番の隔離方式にはしない。** 精度の切分けのためだけに用いた。
- **提出単位のjobでcompileと複数caseの実行はできた。** ただし製品のchecker、ケース木、既存Rust Engineとの接続を実装したものではなく、分散ノード方式との性能比較もしていない。

今回のcaseごとの分離はfresh process・UID・process group・制限の再設定であり、
caseごとに新しいVM／namespaceを作る実装ではない。EC2のgVisor containerは提出ごとに作り直す。
APIはallowlist fixtureしか受け付けず、任意の提出コードや爆弾入力全般の隔離を認証した結果ではない。
提出順を決めるDBの受付契約もこの試験の対象外である。

## 再実行と撤去

起動時スキルと[infra/PATTERNS.md](../../infra/PATTERNS.md)に従い、CDKで作成・削除する。
同名スタックが残っていれば先にdestroyする。共有bootstrapは削除しない。
このセッションでは以下のselectorを使った。他環境では`OIC_MANIFEST_PATH`のAWS entryを選ぶ。

```sh
source /workspace/tooling/env.sh
export AWS_PROFILE=AWS
export AWS_CONFIG_FILE=/run/codex-environment/outbound-identity/aws/config
export npm_config_cache=/workspace/traOJudge/work/npm-cache
cd /workspace/traOJudge
UV_CACHE_DIR="$PWD/work/uv-cache" uv venv work/venv
UV_CACHE_DIR="$PWD/work/uv-cache" uv pip install --python work/venv/bin/python boto3==1.43.123 matplotlib==3.11.2
work/venv/bin/python infra/experiments/fairness/prepare.py
cd infra
npm ci
npx aws-cdk deploy --app 'npx tsx bin/fairness.ts' --output ../work/cdk-fairness --require-approval never --outputs-file ../work/fairness-outputs.json
npx aws-cdk deploy --app 'npx tsx bin/fairness-ec2.ts' --output ../work/cdk-fairness-ec2 --require-approval never --outputs-file ../work/fairness-ec2-outputs.json
npx aws-cdk deploy --app 'npx tsx bin/fairness-queue.ts' --output ../work/cdk-fairness-queue --require-approval never --outputs-file ../work/fairness-queue-outputs.json
cd ..
# EC2のhosts/報告を2件確認し、既存の測定記録と別の出力先で実行する。
work/venv/bin/python infra/experiments/fairness/run.py --target microvm --report work/fairness-microvm.json
work/venv/bin/python infra/experiments/fairness/long_probe.py --target microvm
work/venv/bin/python infra/experiments/fairness/run.py --target ec2 --report work/fairness-ec2.json
work/venv/bin/python infra/experiments/fairness/run.py --target ec2 --runtime runc --precompiled --quick-control --report work/fairness-ec2-native.json
work/venv/bin/python infra/experiments/fairness/long_probe.py --target ec2
work/venv/bin/python infra/experiments/fairness/long_probe.py --target ec2 --runtime runc
# direct MicroVM試験が終わってから実行。3 VMをsuspendするため同時には行わない。
work/venv/bin/python infra/experiments/fairness/queue_run.py
# EC2の全測定が終わってから、idleを確認して実際に0台へ縮小する。
work/venv/bin/python infra/experiments/fairness/cold_ec2.py
work/venv/bin/python infra/experiments/fairness/analyze.py
cd infra
# 成否にかかわらずこの順で削除する。APIで所有リソースの不存在も確認する。
npx aws-cdk destroy --app 'npx tsx bin/fairness-queue.ts' --output ../work/cdk-fairness-queue --force
npx aws-cdk destroy --app 'npx tsx bin/fairness.ts' --output ../work/cdk-fairness --force
npx aws-cdk destroy --app 'npx tsx bin/fairness-ec2.ts' --output ../work/cdk-fairness-ec2 --force
```

この試験の出力は[測定JSON群](fairness-data/)。初回のcompileを省いたgVisor cohortも別ファイルに保存し、
主たる修正版の結果へ混ぜていない。初回の構築失敗・計測器修正は性能sampleに含めていない。
型検査、Python構文検査、ローカルのnative containerでcompile／CPU上限／wall監視／abort／child集計の動作検査も行った。
ローカル検査の時間値はAWSの性能結果には使っていない。
最終cohortのcase測定は1,904件で、うち正常出力・正の時間値を検査したcaseは1,160件。
[データ検査記録](fairness-data-validation.json)に件数・compiler・prebuilt binaryの一致を保存した。

検証用3スタックは削除済み。APIでスタック、6つのLambda function、2 VPC、
MicroVM imageの不存在、3 MicroVMの停止、同じスタック名に属する非terminated EC2が0台であることを確認した。
共有CDKToolkitは保持した。詳細は[撤去確認](fairness-teardown.json)。

構成図は[stack-diagram](https://github.com/caru-ini/stack-diagram)の
`9413ef2b611bb695cb969913c829beda828affaf`を使い、[生成スクリプト](../../infra/scripts/fairness-diagram.py)から作成した。
分布図は測定JSONからMatplotlibで生成した。
