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
- 更新時は `package.json` と `package-lock.json` を一緒に変更し、API生成・lint・型検査・buildと、変更したツールの起動を確認する。

## インストール時のスクリプト

`.npmrc` の `ignore-scripts=true` で、インストール時のスクリプトの自動実行を無効にしてある。Git フックの設定手順は [UI の README](../../UI/README.md#project-setup) を参照する。

## 推移依存の修正版

`basic-ftp` の脆弱性対応として、`get-uri` 経由の依存を `overrides` で上書きしてある。`get-uri` の要求範囲を超えるため、更新時は互換性を確認する。上書きなしで修正版を利用できるようになったら、この設定を削除する。

根拠: [basic-ftp のアドバイザリ](https://github.com/advisories/GHSA-c475-qrg2-pj4r)。

## API生成

OpenAPI Generatorはnpmの依存とは別にJARを取得する。JARの版は `UI/openapitools.json` で指定し、npmのlockfile・公開後の待機期間の対象外。通常問題のスキーマは現在 `UI/scripts/gen-api.sh` が旧ドキュメントリポジトリのdevelopを参照し、コンテストトップは同じリポジトリの `docs/API/front_back.yaml` を使う。
