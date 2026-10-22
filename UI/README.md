# traO-Judge-frontend

traP Online Judge フロントエンド

Node の版は `.node-version`、npm の版は `package.json` の
`devEngines.packageManager.version` に合わせてください。
依存の固定・インストール時のスクリプト制限は[依存の管理](../docs/ui/dependencies.md)を参照してください。

## Recommended IDE Setup

[VSCode](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) (and disable Vetur).

## Type Support for `.vue` Imports in TS

TypeScript cannot handle type information for `.vue` imports by default, so we replace the `tsc` CLI with `vue-tsc` for type checking. In editors, we need [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) to make the TypeScript language service aware of `.vue` types.

## Customize configuration

See [Vite Configuration Reference](https://vitejs.dev/config/).

## Project Setup

```sh
npm ci
npm run prepare
./scripts/gen-api.sh development
```

コマンドは `UI/` で実行します。`npm ci` ではインストール時のスクリプトを実行しないため、
Git フックは `npm run prepare` で明示的に設定します。
この設定は同じリポジトリの他のworktreeにも適用されます。

### Compile and Hot-Reload for Development

```sh
npm run dev
```

### Type-Check, Compile and Minify for Production

```sh
npm run build
```

### Lint with [ESLint](https://eslint.org/)

```sh
npm run lint
```

## API周りのコード生成の仕方

`UI/` で `./scripts/gen-api.sh development` を実行してください。通常問題の旧契約と、コンテストトップの新契約をそれぞれ生成します。APIを利用する部分については，生成されたコードを使って実装をしてください。

## mockサーバーの立ち上げ方

開発環境においては，モックサーバーを利用します。モックサーバーの立ち上げには`./scripts/run-mock-api.sh`を実行してください。
このファイルを実行すると`localhost:4010`にモックサーバーが立ち上がりますので，ここで提供されるAPIを叩いてください。
