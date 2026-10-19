# Rust の依存関係管理

バックエンドのワークスペースでは [cargo-hakari](https://docs.rs/cargo-hakari/latest/cargo_hakari/) で依存クレートの feature を統一し、クレートを切り替えてビルドするときの再コンパイルを減らす。
設定は `.config/hakari.toml`、生成先は `server/workspace-hack/Cargo.toml`。`workspace-hack` はビルド用の補助クレートで、業務ロジックは置かない。

## 開発環境

リポジトリのルートで `nix develop` を実行する。`flake.lock` で固定された cargo-hakari 0.9.36 と、`rust-toolchain.toml` に対応する Rust が使える。
Nix を使わない場合は、Rust を用意したうえで次のコマンドで同じバージョンをインストールする。

```sh
cargo install cargo-hakari --version 0.9.36 --locked
```

## 依存関係を変更したとき

クレートの追加・削除、依存関係の feature の変更、`cargo update` の後は、リポジトリのルートで次を実行する。

```sh
cargo hakari generate
cargo hakari manage-deps --yes
```

生成された `server/workspace-hack/Cargo.toml`、各クレートの `Cargo.toml`、`Cargo.lock` を変更と一緒にコミットする。生成区間を手で編集しない。
新しいワークスペースメンバーにも `workspace-hack.workspace = true` が追加される。

CI と同じ整合性チェックは次のコマンドで実行できる。

```sh
cargo hakari generate --diff
cargo hakari manage-deps --dry-run
cargo hakari verify
```

cargo-hakari のバージョンを更新するときは、Nix 環境・`.github/workflows/cargo-hakari.yaml`・この手順のバージョンを揃え、再生成とチェックを実行する。
