use super::{ContestService, permissions::*};
use crate::{
    domain::model::{
        account::{AccountRole, AuthenticatedSession, RoleName},
        contest::*,
        user::UserId,
    },
    ports::repository::contest::{ContestUnitOfWork, ContestUnitOfWorkProvider},
};
use uuid::Uuid;
impl<U: ContestUnitOfWorkProvider> ContestService<U> {
    pub async fn grant_role(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        target: UserId,
        role: RoleName,
    ) -> Result<(AccountRole, bool), ContestError> {
        valid_role(kind, role)?;
        let mut tx = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(kind, id).with_user(target),
            )
            .await?;
        let permitted = tx
            .has_service_role(actor.user_id, RoleName::ServiceAdmin)
            .await?
            || (kind == ContestKind::Regular
                && role == RoleName::ContestSetter
                && has_role(&mut tx, actor.user_id, RoleName::ContestAdmin).await?);
        if !permitted {
            return Err(ContestError::Forbidden);
        }
        let (assignment, changed) = match tx.assignment(target, role).await? {
            Some(assignment) => (assignment, false),
            None => (tx.insert_role(target, role).await?, true),
        };
        tx.commit().await?;
        Ok((assignment, changed))
    }
    pub async fn revoke_role(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        target: UserId,
        role: RoleName,
    ) -> Result<(), ContestError> {
        valid_role(kind, role)?;
        let mut tx = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(kind, id).with_user(target),
            )
            .await?;
        require_service_admin(&mut tx, actor.user_id).await?;
        if role == kind.participant() {
            tx.remove_membership(target).await?;
        }
        tx.delete_role(target, role).await?;
        tx.commit().await
    }
}
fn valid_role(kind: ContestKind, role: RoleName) -> Result<(), ContestError> {
    if role == kind.setter()
        || role == kind.participant()
        || (kind == ContestKind::Regular && role == RoleName::ContestAdmin)
    {
        Ok(())
    } else {
        Err(ContestError::Forbidden)
    }
}
