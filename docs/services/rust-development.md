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

## CI のビルドとキャッシュ

`.github/workflows/cargo-build.yaml` は Rust のソース・依存関係・ビルド設定を変更した PR と `develop` / `main` への push で動く。手動実行もできる。
`rust-toolchain.toml` のバージョンを読み、Ubuntu 24.04 上で `cargo build --workspace --all-targets --locked` を実行する。テスト用の実行ファイルもビルドするが、テストの実行は含まない。

[Swatinem/rust-cache](https://github.com/Swatinem/rust-cache#cache-details) をコミット SHA で固定し、Cargo registry・Git 依存・`target` 内の依存クレートの成果物を保存する。
OS の世代・アーキテクチャ・Rust のバージョン・Cargo 用環境変数でキャッシュを分離し、manifest と lockfile の変更もキーに反映する。ソースやコミット SHA はキーに含めず、ソースだけの変更では同じ依存キャッシュを再利用する。依存更新時も同じビルド環境のキャッシュを部分復元し、Cargo が変更分を再ビルドする。

キャッシュ保存時には、ワークスペース自身の成果物・incremental データ・不要になった依存を除く。CI では incremental compilation とデバッグ情報を無効にして、生成量と転送量を減らす。ローカル開発でのプロファイル設定は変えない。
保存は `develop` / `main` の成功時に限り、PR は共有ブランチのキャッシュを読み取る。PR の更新時は古い実行をキャンセルする。

ローカルで CI と同じビルド条件を試す場合は、次を実行する。

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo build --workspace --all-targets --locked
```

キャッシュの効果は Actions ログの `Cache Configuration`、復元・保存ログ、Cargo のビルド時間で確認する。初回は `develop` / `main` で成功させて共有キャッシュを作る。キャッシュがなくても全ターゲットのビルドは必ず実行する。
