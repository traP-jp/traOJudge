use crate::domain::model::{
    account::{AccountRole, AuthenticatedSession, RoleName},
    editorial::*,
    problem::ProblemReference,
    user::UserId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait EditorialUnitOfWorkProvider: Send + Sync {
    type UnitOfWork<'a>: EditorialUnitOfWork
    where
        Self: 'a;
    /// 本人と権限変更対象を一定順序で直列化し、セッションを再検査する。
    /// 取得した解説はUoW終了まで更新・権限変更に対して直列化する。
    async fn begin(
        &self,
        actor: Option<AuthenticatedSession>,
        affected_users: &[UserId],
    ) -> Result<Self::UnitOfWork<'_>, EditorialError>;
}

/// 業務上の判断・操作の順序・commitはapplicationが担う。未確定での破棄はrollback。
#[async_trait]
pub trait EditorialUnitOfWork: Send + Sized {
    fn now(&self) -> DateTime<Utc>;
    async fn has_service_role(
        &mut self,
        user_id: UserId,
        role: RoleName,
    ) -> Result<bool, EditorialError>;
    /// 検査後もUoW終了まで親の公開状態変更・削除を待たせる。
    async fn problem_reference(
        &mut self,
        id: Uuid,
    ) -> Result<Option<ProblemReference>, EditorialError>;
    async fn has_problem_role(
        &mut self,
        user_id: UserId,
        problem_id: Uuid,
        role: RoleName,
    ) -> Result<bool, EditorialError>;
    async fn find(&mut self, id: Uuid) -> Result<Option<Editorial>, EditorialError>;
    async fn list_public(
        &mut self,
        problem_id: Uuid,
        page: &EditorialPageRequest,
    ) -> Result<Vec<Editorial>, EditorialError>;
    async fn insert(&mut self, editorial: &Editorial) -> Result<(), EditorialError>;
    async fn save(&mut self, editorial: &Editorial) -> Result<(), EditorialError>;
    async fn save_publication(&mut self, editorial: &Editorial) -> Result<(), EditorialError>;
    async fn delete(&mut self, id: Uuid) -> Result<(), EditorialError>;
    async fn author_assignment(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<Option<AccountRole>, EditorialError>;
    async fn insert_author(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<AccountRole, EditorialError>;
    async fn delete_author(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<(), EditorialError>;
    async fn creation_receipt(
        &mut self,
        user_id: UserId,
        key: &EditorialCreationKey,
    ) -> Result<Option<EditorialCreationReceipt>, EditorialError>;
    async fn save_creation_receipt(
        &mut self,
        key: &EditorialCreationKey,
        receipt: &EditorialCreationReceipt,
        expires_at: DateTime<Utc>,
    ) -> Result<(), EditorialError>;
    async fn commit(self) -> Result<(), EditorialError>;
    async fn rollback(self) -> Result<(), EditorialError>;
}
