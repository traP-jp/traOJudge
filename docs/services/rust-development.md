# Rust の依存関係管理

バックエンドのワークスペースでは [cargo-hakari](https://docs.rs/cargo-hakari/latest/cargo_hakari/) で依存クレートの feature を統一し、クレートを切り替えてビルドするときの再コンパイルを減らす。
設定は `.config/hakari.toml`、生成先は `server/workspace-hack/Cargo.toml`。`workspace-hack` はビルド用の補助クレートで、業務ロジックは置かない。

## 開発環境

リポジトリのルートで `nix develop` を実行する。`flake.lock` で固定された cargo-hakari 0.9.36 と、`rust-toolchain.toml` に対応する Rust が使える。
Nix を使わない場合は、Rust を用意したうえで次のコマンドで同じバージョンをインストールする。

```sh
cargo install cargo-hakari --version 0.9.36 --locked
```

## 依存クレートの公開から更新までの待機期間

`.cargo/config.toml` の `registry.global-min-publish-age = "7 days"` と
`resolver.incompatible-publish-age = "deny"` により、依存関係の解決では公開から7日未満のバージョンを候補から除外する。
`cargo update` や `cargo add` に追加のフラグは不要。

この設定に対応する Cargo を使うため、`rust-toolchain.toml` は `nightly-2026-10-25` に固定する。
Rustup・Nix・CI はこのファイルを参照する。`rust-toolchain.toml` はツールチェーンを選ぶためのファイルであり、待機期間の設定は `.cargo/config.toml` に置く。
この nightly には Rust 1.100 で安定化される機能が含まれるため、`-Z min-publish-age` や `[unstable]` は不要。
詳細は [Cargo の設定リファレンス](https://doc.rust-lang.org/nightly/cargo/reference/config.html#registryglobal-min-publish-age) を参照する。

すでに `Cargo.lock` に固定されているバージョンは待機期間の対象外で、既存の依存関係は自動で巻き戻さない。
公開日時を提供しないレジストリや Git・path 依存にはこの制限は適用されない。
明示的な別ツールチェーンの指定や Cargo の設定上書きは可能なので、上書きせずリポジトリ指定の環境を使う。

緊急のセキュリティ修正など、7日を待たず特定のバージョンへ更新する必要がある場合だけ、次のようにその実行に限って制限を解除する。

```sh
CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow \
  cargo update -p <crate> --precise <version>
```

その場合も、下記の cargo-hakari の再生成と整合性チェックを行い、変更した `Cargo.lock` をレビューする。

Dependabot の Cargo の通常更新にも `.github/dependabot.yml` で7日の `cooldown` を設定する。
[Dependabot の cooldown](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference#cooldown) はセキュリティ更新には適用されない。

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
