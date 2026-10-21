use async_trait::async_trait;

/// Storage backendで一連の変更を確定・破棄する単位。
///
/// 未確定のまま破棄された場合、実装は変更が確定されないことを保証する。
/// 確定・破棄は所有権を消費し、同じUnit of Workを再利用できない。
///
/// ```compile_fail
/// use traojudge_workflow::storage::UnitOfWork;
///
/// async fn commit_twice<Work: UnitOfWork>(work: Work) -> Result<(), Work::Error> {
///     work.commit().await?;
///     work.commit().await
/// }
/// ```
#[async_trait]
pub trait UnitOfWork: Send + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn commit(self) -> Result<(), Self::Error>;

    async fn rollback(self) -> Result<(), Self::Error>;
}

/// Storage backendのUnit of Workを開始する。
///
/// Unit of Workはproviderを借用でき、接続などの寿命をbackend側で表現できる。
/// 開始・確定・破棄のfutureは`Send`で、非同期ランタイム上で利用できる。
///
/// ```
/// use traojudge_workflow::storage::{UnitOfWork, UnitOfWorkProvider};
///
/// async fn commit_work<Provider: UnitOfWorkProvider>(
///     provider: &Provider,
/// ) -> Result<(), Provider::Error> {
///     let work = provider.begin().await?;
///     work.commit().await
/// }
/// ```
#[async_trait]
pub trait UnitOfWorkProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    type UnitOfWork<'provider>: UnitOfWork<Error = Self::Error>
    where
        Self: 'provider;

    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, Self::Error>;
}
