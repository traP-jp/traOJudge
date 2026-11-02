# Lambdaの作成・呼出し・削除の最小再現

`develop-hayatroid` (`d2400cb`) から `codex-judge-fairness` を作成した。
東京リージョンでCDKからLambda・実行ロール・ログの3リソースを作成し、
異なる2つのJSONを同期呼出しで送り、入力と応答の完全一致を検証した。
その後CDK destroyを実行し、スタック・関数・ロール・ログの不存在を各APIで確認した。

- [実測記録](lambda-smoke-evidence.json)：両方HTTP 200。クライアントからの計測は1.2688秒／1.0548秒。
- [構成図（明）](diagrams/lambda-smoke.png)・[構成図（暗）](diagrams/lambda-smoke-dark.png)・[自己完結HTML](diagrams/lambda-smoke.html)
- [CDK定義](../../infra/bin/lambda-smoke.ts)・[再実行スクリプト](../../infra/scripts/lambda-smoke.py)

これはAWSへの作成・呼出し・削除を確認する疎通試験であり、ジャッジ性能、隔離、耐障害性、公平性の試験ではない。
計測値はネットワークとSDK処理を含み、cold start／warm startの分離計測ではない。

## 再実行

環境の起動時スキルおよび `infra/PATTERNS.md` に従う。AWS profileは、
`OIC_MANIFEST_PATH` の `AWS` entryを選ぶ。認証値は読まず、設定ファイルをselectorに使う。

```sh
source /workspace/tooling/env.sh
export AWS_PROFILE=AWS
export AWS_CONFIG_FILE=/run/codex-environment/outbound-identity/aws/config
export npm_config_cache=/workspace/traOJudge/work/npm-cache
cd /workspace/traOJudge
UV_CACHE_DIR="$PWD/work/uv-cache" uv venv work/venv
UV_CACHE_DIR="$PWD/work/uv-cache" uv pip install --python work/venv/bin/python boto3==1.43.123
cd infra
npm ci
# 初回のみ。既存の共有CDKToolkitは削除しない。
# npx aws-cdk bootstrap
../work/venv/bin/python scripts/lambda-smoke.py --report ../work/lambda-smoke-rerun.json
```

スクリプトは同じブランチ名のsmokeスタックが残っていれば先にdestroyし、
deploy → invoke 2件 → finallyでdestroy → 不存在確認を行う。
今回の実行では、既存CDKToolkitがCREATE_COMPLETEだったためbootstrapは追加実行していない。
専用smokeスタックは削除済み。AWSアカウントID・ARN・リソースURLは公開記録に含めない。

## 構成図の生成

[caru-ini/stack-diagram](https://github.com/caru-ini/stack-diagram) の
`9413ef2b611bb695cb969913c829beda828affaf` にあるtemplate、fetch_icons.py、build.py、render_png.pyを使用。
AWS公式アイコンを埋め込み、同じHTMLから明暗のPNGを生成して目視確認した。
環境のChromiumを指定し、file URLの代わりにHTML文字列を読み込む実行環境向けの調整を行った。
図はこの疎通試験の構成である。
