# UI の依存管理

Node は `UI/.node-version`、npm は `UI/package.json` の `devEngines.packageManager.version` に合わせる。CI もこの指定を使う。

指定の npm が入っていない場合は、指定の Node を選択した後、`UI/` で次を実行する。

```sh
ui_npm_version=$(node -p "require('./package.json').devEngines.packageManager.version")
ui_release_age=$(npm config get min-release-age)
npm install --global "npm@$ui_npm_version" --ignore-scripts --min-release-age="$ui_release_age"
```

## インストールと更新

- 通常のセットアップ・CI は `npm ci` で lockfile の解決結果を再現する。
- 直接依存は完全なバージョン番号で指定する。`.npmrc` の `save-exact=true` により、追加時も範囲指定にしない。
- `.npmrc` の `min-release-age=3` は、新しく依存を解決するときに公開後3日の待機期間を設ける。既存の lockfile 全体を検査する機能ではないため、lockfile の更新内容も確認する。
- 更新時は `package.json` と `package-lock.json` を一緒に変更し、[開発手順](development.md)に沿ってAPI生成・lint・テスト・buildと、変更したツールの起動を確認する。

## インストール時のスクリプト

`.npmrc` の `ignore-scripts=true` で、インストール時のスクリプトの自動実行を無効にしてある。Git フックの設定は[セットアップ](development.md#セットアップ)を参照する。

## 対応ブラウザー

Tailwind CSS 4の対応条件に合わせ、Safari 16.4、Chrome 111、Firefox 128以降を対象とする。[公式のブラウザー要件](https://tailwindcss.com/docs/upgrade-guide#browser-requirements)を参照。

## 推移依存の修正版

`basic-ftp` の脆弱性対応として、`get-uri` 経由の依存を `overrides` で上書きしてある。`get-uri` の要求範囲を超えるため、更新時は互換性を確認する。上書きなしで修正版を利用できるようになったら、この設定を削除する。

根拠: [basic-ftp のアドバイザリ](https://github.com/advisories/GHSA-c475-qrg2-pj4r)。

## GitHub ActionsとAPI生成

UI用Actionsは40桁の commit SHA に固定し、コメントに版を書く。checkoutの認証情報は保存せず、workflowの権限は `contents: read` とする。既存のDependabot設定を利用する。

UIのCIはbase branchを限定しない。`UI/`、`docs/API/`、`docs/ui/` とworkflow自身の変更で実行し、通常問題・コンテスト両方のAPI生成、lint、テスト、型検査、buildを確認する。

OpenAPI Generatorはnpmの依存とは別にJARを取得する。JARの版は `UI/openapitools.json` で指定し、npmのlockfile・公開後の待機期間の対象外。通常問題のスキーマは、`UI/scripts/gen-api.sh` の `SCHEMA_COMMIT` で指定した旧ドキュメントリポジトリのcommitから取得する。コンテストトップは、このモノレポの `docs/API/front_back.yaml` を使う。
