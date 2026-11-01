use crate::domain::model::{
    account::{AccountRole, AuthenticatedSession, RoleName},
    contest::*,
    problem::ProblemReference,
    request::IdempotencyKey,
    user::UserId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait ContestUnitOfWorkProvider: Send + Sync {
    type UnitOfWork<'a>: ContestUnitOfWork
    where
        Self: 'a;
    /// 対象大会を先に直列化し、影響する利用者をUUID順にロックしてセッションを再検査する。
    async fn begin(
        &self,
        actor: Option<AuthenticatedSession>,
        scope: ContestScope,
    ) -> Result<Self::UnitOfWork<'_>, ContestError>;
}
#[async_trait]
pub trait ContestUnitOfWork: Send + Sized {
    fn now(&self) -> DateTime<Utc>;
    fn current(&self) -> Result<Contest, ContestError>;
    async fn has_service_role(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<bool, ContestError>;
    async fn assignment(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<Option<AccountRole>, ContestError>;
    async fn insert_role(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<AccountRole, ContestError>;
    async fn delete_role(&mut self, user: UserId, role: RoleName) -> Result<(), ContestError>;
    async fn is_active(&mut self, kind: ContestKind, id: Uuid) -> Result<bool, ContestError>;
    async fn insert(&mut self, contest: &Contest) -> Result<(), ContestError>;
    async fn save(&mut self, contest: &Contest) -> Result<(), ContestError>;
    async fn delete(&mut self) -> Result<(), ContestError>;
    async fn list_public(
        &mut self,
        kind: ContestKind,
        page: &ContestPageRequest,
    ) -> Result<Vec<Contest>, ContestError>;
    async fn clear_participation(&mut self) -> Result<(), ContestError>;
    async fn problem(&mut self, id: Uuid) -> Result<Option<ProblemReference>, ContestError>;
    async fn problems(&mut self) -> Result<Vec<ContestProblem>, ContestError>;
    /// 削除された親問題の出題枠も含め、使用済みの順序を返す。
    async fn max_problem_order(&mut self) -> Result<Option<i32>, ContestError>;
    async fn insert_problem(&mut self, problem: &ContestProblem) -> Result<(), ContestError>;
    async fn remove_problem(&mut self, id: Uuid) -> Result<(), ContestError>;
    async fn team_id(
        &mut self,
        name: &str,
        password_hash: &str,
    ) -> Result<Option<Uuid>, ContestError>;
    async fn create_team(
        &mut self,
        id: Uuid,
        name: &str,
        password_hash: &str,
    ) -> Result<(), ContestError>;
    async fn team(&mut self, id: Uuid) -> Result<Option<ContestTeam>, ContestError>;
    async fn membership(
        &mut self,
        user: UserId,
    ) -> Result<Option<(Uuid, DateTime<Utc>)>, ContestError>;
    async fn set_membership(
        &mut self,
        user: UserId,
        team: Uuid,
    ) -> Result<DateTime<Utc>, ContestError>;
    async fn remove_membership(&mut self, user: UserId) -> Result<(), ContestError>;
    async fn receipt(
        &mut self,
        user: UserId,
        operation: &str,
        scope: Uuid,
        key: &IdempotencyKey,
    ) -> Result<Option<ContestReceipt>, ContestError>;
    async fn save_receipt(
        &mut self,
        user: UserId,
        operation: &str,
        scope: Uuid,
        key: &IdempotencyKey,
        receipt: &ContestReceipt,
        expires_at: DateTime<Utc>,
    ) -> Result<(), ContestError>;
    async fn commit(self) -> Result<(), ContestError>;
    async fn rollback(self) -> Result<(), ContestError>;
}
