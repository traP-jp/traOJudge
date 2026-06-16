pub mod auth;
pub mod editorial;
pub mod icon;
pub mod language;
pub mod problem;
pub mod session;
pub mod submission;
pub mod testcase;
pub mod user;

use super::service::unit_of_work_provider::{UnitOfWork, UnitOfWorkProvider};

/// Provides repositories backed by the ordinary infra pool.
///
/// Repository values returned from this provider are independent from each
/// other. In SQL infra, these repositories usually own a cloned pool handle.
pub trait RepositoryProvider: Send + Sync {
    type AuthRepository: auth::AuthRepository;
    type EditorialRepository: editorial::EditorialRepository;
    type IconRepository: icon::IconRepository;
    type LanguageRepository: language::LanguageRepository;
    type ProblemRepository: problem::ProblemRepository;
    type SessionRepository: session::SessionRepository;
    type SubmissionRepository: submission::SubmissionRepository;
    type TestcaseRepository: testcase::TestcaseRepository;
    type UserRepository: user::UserRepository;

    fn provide_auth_repository(&self) -> Self::AuthRepository;
    fn provide_editorial_repository(&self) -> Self::EditorialRepository;
    fn provide_icon_repository(&self) -> Self::IconRepository;
    fn provide_language_repository(&self) -> Self::LanguageRepository;
    fn provide_problem_repository(&self) -> Self::ProblemRepository;
    fn provide_session_repository(&self) -> Self::SessionRepository;
    fn provide_submission_repository(&self) -> Self::SubmissionRepository;
    fn provide_testcase_repository(&self) -> Self::TestcaseRepository;
    fn provide_user_repository(&self) -> Self::UserRepository;
}

/// Provides repositories backed by a single unit of work.
///
/// This is intended for infra types that own a transaction. Each repository
/// value returned from this trait can borrow the unit of work mutably and use
/// that transaction as its executor. Because of that mutable borrow, callers
/// should keep one repository value alive at a time:
///
/// ```ignore
/// let mut uow = provider.begin().await?;
///
/// let problem_id = uow
///     .provide_problem_repository()
///     .create_problem(problem)
///     .await?;
///
/// uow.provide_testcase_repository()
///     .create_testcases(testcases)
///     .await?;
///
/// uow.commit().await?;
/// # anyhow::Ok(())
/// ```
pub trait RepositoryUnitOfWork: UnitOfWork {
    type AuthRepository<'a>: auth::AuthRepository + 'a
    where
        Self: 'a;
    type EditorialRepository<'a>: editorial::EditorialRepository + 'a
    where
        Self: 'a;
    type IconRepository<'a>: icon::IconRepository + 'a
    where
        Self: 'a;
    type LanguageRepository<'a>: language::LanguageRepository + 'a
    where
        Self: 'a;
    type ProblemRepository<'a>: problem::ProblemRepository + 'a
    where
        Self: 'a;
    type SessionRepository<'a>: session::SessionRepository + 'a
    where
        Self: 'a;
    type SubmissionRepository<'a>: submission::SubmissionRepository + 'a
    where
        Self: 'a;
    type TestcaseRepository<'a>: testcase::TestcaseRepository + 'a
    where
        Self: 'a;
    type UserRepository<'a>: user::UserRepository + 'a
    where
        Self: 'a;

    fn provide_auth_repository(&mut self) -> Self::AuthRepository<'_>;
    fn provide_editorial_repository(&mut self) -> Self::EditorialRepository<'_>;
    fn provide_icon_repository(&mut self) -> Self::IconRepository<'_>;
    fn provide_language_repository(&mut self) -> Self::LanguageRepository<'_>;
    fn provide_problem_repository(&mut self) -> Self::ProblemRepository<'_>;
    fn provide_session_repository(&mut self) -> Self::SessionRepository<'_>;
    fn provide_submission_repository(&mut self) -> Self::SubmissionRepository<'_>;
    fn provide_testcase_repository(&mut self) -> Self::TestcaseRepository<'_>;
    fn provide_user_repository(&mut self) -> Self::UserRepository<'_>;
}

pub trait RepositoryUnitOfWorkProvider: UnitOfWorkProvider
where
    Self::UnitOfWork: RepositoryUnitOfWork,
{
}

impl<T> RepositoryUnitOfWorkProvider for T
where
    T: UnitOfWorkProvider,
    T::UnitOfWork: RepositoryUnitOfWork,
{
}
