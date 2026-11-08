# AWS ジャッジ実行の構成図

`aws-judge.html` はフォント・アイコン・データを含む、オフラインで開ける構成図。ブラウザで開き、タブ・方式・段階を切り替えられる。ノードから実装を参照できる。AWS配置の静止画は[light](aws-judge.png) / [dark](aws-judge-dark.png)。いずれも完成HTMLの`.canvas`をChromiumで2倍解像度に撮影している。

- AWSの配置と権限境界（runtimeの通信は次のレーン図に分離）
- ファイルと実行の二部DAG、およびそこから導くJob依存
- 要求・結果・ファイル転送を別の直線レーンで示す説明再生
- interactive / coding-gameの直線FIFO通信、コンテナ別の入力権限、mainが生成する結果・replay
- 通常判定・heuristic・run-twice・interactive・coding-gameの構成
- AWSで保存した実際の監査イベントと結果

[実装と再現手順](../aws-judge.md)を参照。

## 再生成

[caru-ini/stack-diagram](https://github.com/caru-ini/stack-diagram)を作業用ディレクトリに取得し、同スキルの`SKILL.md`と`references/geometry.md`を読む。公式AWSアイコンとLucideを、同スキルの`fetch_icons.py`で取得する。取得したパッケージや単体アイコンはリポジトリに追加しない。

```sh
python3 docs/services/stack-diagram/build.py \
  --skill /tmp/stack-diagram/skills/stack-diagram \
  --icons /tmp/judge-icons \
  --draft /tmp/judge-draft.html \
  --proof docs/services/aws-judge-verification.json
```

`build.py`はスキルのテンプレートと`build.py`を使ってアセットを埋め込む。同じアイコンのdata URIは一度だけ保存して、ブラウザで各ノードに割り当てる。PNGはこの完成したHTMLをChromiumで表示したものから生成する。別の描画方式で作り直さない。

AWS検証のJSONにはアカウントID・実リソースID・ARN・URLを含めず、判定結果とイベントの必要な項目だけを載せる。`--proof`を省略した場合は未検証と表示する。`aws_verified`がJSONの`true`である場合だけ「AWS検証済み」と表示する。判定方式タブの値は実行例の期待値であり、実測値は記録タブで確認する。完成HTMLが256 KiBを超えた場合は生成を失敗させる。

## 図の読み方と確認

AWS配置図はサービスと配置境界を示し、runtimeの全接続を重ねない。「制御とデータ」は同じController / Workerをレーンごとに再掲する。青はTask・NodeResult・永続受理、橙はファイル内容とArtifact依存、灰色の破線はExecution内のFIFO会話。sandboxからS3 / SQSへ直接つなぐ線はない。

session図ではmainとpeerのコンテナ境界を分け、判定JSONとtranscript / replayの生成元をmainの内側に置く。各peerは専用FIFO対と入力集合を持つ。Artifact、Execution定義、Engineが管理するJobは別の概念として二部DAGタブで確認できる。

変更時はChromiumで全6タブと5方式をlight / darkの両方で表示する。タブ、方式selector、ノード詳細リンク、8段階の再生、狭い画面の横スクロールを確認する。`file://`が制限された環境ではPlaywrightの`page.set_content()`に完成HTMLを渡し、`document.fonts.ready`と画像の読み込みを待ってから`.canvas`を撮影する。

## ライセンス

図内のAWSサービスアイコンはAWS Architecture Iconsを使う。AWSのアイコンパッケージや単体ファイルは配布しない。ブランドの権利は各所有者に帰属する。一般概念のアイコンはLucide。フォントと生成コードのライセンスは`licenses/`に置く。
