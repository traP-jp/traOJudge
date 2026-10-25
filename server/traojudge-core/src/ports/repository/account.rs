use crate::domain::model::{
    account::*,
    user::{PublicUserProfile, UserId},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// WebセッションとCLIトークンの照合。アカウント変更のtransactionとは独立する。
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn refresh_session(
        &self,
        digest: &[u8],
    ) -> Result<Option<AuthenticatedSession>, AccountError>;
    async fn developer_token_user(&self, digest: &[u8]) -> Result<Option<UserId>, AccountError>;
}

#[async_trait]
pub trait AccountUnitOfWorkProvider: Send + Sync {
    type UnitOfWork<'a>: AccountUnitOfWork
    where
        Self: 'a;
    /// 本人の行をロックし、セッションの有効性を再検査する。
    /// 未確定のtransactionは破棄時にrollbackする。
    async fn begin(
        &self,
        actor: AuthenticatedSession,
    ) -> Result<Self::UnitOfWork<'_>, AccountError>;
}

/// 1人のアカウントに対する永続化操作。業務判断とcommitの順序はapplicationが決める。
/// 所有者の絞り込みとDB制約は各実装でも維持する。
#[async_trait]
pub trait AccountUnitOfWork: Send + Sized {
    fn now(&self) -> DateTime<Utc>;
    async fn current_account(&mut self) -> Result<CurrentAccount, AccountError>;
    async fn save_profile(&mut self, profile: &PublicUserProfile) -> Result<(), AccountError>;
    async fn sign_in_methods(&mut self) -> Result<Vec<SignInMethod>, AccountError>;
    async fn delete_sign_in_method(&mut self, id: Uuid) -> Result<(), AccountError>;
    async fn developer_tokens(&mut self) -> Result<Vec<DeveloperToken>, AccountError>;
    async fn insert_developer_token(
        &mut self,
        token: NewDeveloperToken,
    ) -> Result<DeveloperToken, AccountError>;
    async fn revoke_developer_token(&mut self, id: Uuid) -> Result<bool, AccountError>;
    async fn email_owner(&mut self, email: &str) -> Result<Option<UserId>, AccountError>;
    async fn replace_email_confirmation(
        &mut self,
        email: &str,
        digest: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), AccountError>;
    async fn email_confirmation(
        &mut self,
        digest: &[u8],
    ) -> Result<Option<EmailConfirmation>, AccountError>;
    async fn consume_email_confirmation(&mut self, digest: &[u8]) -> Result<(), AccountError>;
    async fn set_email(&mut self, email: &str) -> Result<(), AccountError>;
    async fn revoke_other_sessions(&mut self) -> Result<(), AccountError>;
    async fn insert_authorization(
        &mut self,
        attempt: AuthorizationAttempt,
    ) -> Result<(), AccountError>;
    async fn authorization(
        &mut self,
        id: Uuid,
    ) -> Result<Option<AuthorizationAttempt>, AccountError>;
    async fn external_identity(
        &mut self,
        provider: Provider,
        subject: &str,
    ) -> Result<Option<(UserId, SignInMethod)>, AccountError>;
    async fn insert_external_method(
        &mut self,
        provider: Provider,
        subject: &str,
    ) -> Result<SignInMethod, AccountError>;
    async fn finish_authorization(&mut self, id: Uuid, method_id: Uuid)
    -> Result<(), AccountError>;
    async fn commit(self) -> Result<(), AccountError>;
    async fn rollback(self) -> Result<(), AccountError>;
}
