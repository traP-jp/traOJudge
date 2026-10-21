use std::path::Path;

use async_trait::async_trait;

/// 実体化方法の記録を解釈して、指定されたパスにファイルを書き込む処理。
///
/// [`Self::Specification`]が方法を記録する値の型で、このtraitの実装が実体化する側となる。
/// DAGには`Self::Specification`の値を保持し、対象の問題IDとともに対応する実装へ渡す。
/// [`Self::realise`]が成功した場合、`destination`へのファイル内容の書き込みが完了している。
#[async_trait]
pub trait Realiser: Send + Sync {
    type Specification: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn realise(
        &self,
        // TODO: 問題IDのnewtypeが別の変更で導入されたら、&strからその型に差し替える。
        problem_id: &str,
        specification: &Self::Specification,
        destination: &Path,
    ) -> Result<(), Self::Error>;
}
