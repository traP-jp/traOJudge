# judge-command

開発用の`flake.lock`と言語環境定義用の`flake.lock`を別々のサイクルでアップデートするため、言語環境は`judge-command/`で管理しています。

このドキュメントのコマンドはすべてプロジェクトルート(`../`)で実行する前提です。

## コマンド

### CI

```
nix flake check ./judge-command
```

### Pre-build

```
nix build ./judge-command#prebuild_<lang-name>
```

```
nix build ./judge-command#prebuild-all
```