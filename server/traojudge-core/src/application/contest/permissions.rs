use crate::{
    domain::model::{
        account::{AuthenticatedSession, RoleName},
        contest::*,
        user::UserId,
    },
    ports::repository::contest::ContestUnitOfWork,
};

pub(super) async fn require_service_admin<T: ContestUnitOfWork>(
    tx: &mut T,
    user: UserId,
) -> Result<(), ContestError> {
    if tx.has_service_role(user, RoleName::ServiceAdmin).await? {
        Ok(())
    } else {
        Err(ContestError::Forbidden)
    }
}
pub(super) async fn has_role<T: ContestUnitOfWork>(
    tx: &mut T,
    user: UserId,
    role: RoleName,
) -> Result<bool, ContestError> {
    Ok(tx.assignment(user, role).await?.is_some())
}
pub(super) async fn require_setter<T: ContestUnitOfWork>(
    tx: &mut T,
    user: UserId,
    c: &Contest,
) -> Result<(), ContestError> {
    if has_role(tx, user, c.kind.setter()).await? {
        Ok(())
    } else {
        Err(ContestError::Forbidden)
    }
}
pub(super) async fn can_view<T: ContestUnitOfWork>(
    tx: &mut T,
    actor: Option<AuthenticatedSession>,
    c: &Contest,
) -> Result<bool, ContestError> {
    if c.visibility == ContestVisibility::Public {
        return Ok(true);
    }
    if let Some(actor) = actor {
        if has_role(tx, actor.user_id, c.kind.setter()).await? {
            return Ok(true);
        }
        if c.kind == ContestKind::Regular
            && has_role(tx, actor.user_id, RoleName::ContestAdmin).await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) async fn require_joinable<T: ContestUnitOfWork>(
    tx: &mut T,
    user: UserId,
    c: &Contest,
) -> Result<(), ContestError> {
    if c.visibility != ContestVisibility::Public {
        return Err(ContestError::NotFound);
    }
    if !tx.has_service_role(user, RoleName::StandardUser).await? {
        return Err(ContestError::Forbidden);
    }
    Ok(())
}
pub(super) fn check_receipt(
    receipt: &ContestReceipt,
    digest: &[u8],
    epoch: Uuid,
) -> Result<(), ContestError> {
    if receipt.digest != digest || receipt.epoch != epoch {
        return Err(ContestError::Conflict);
    }
    Ok(())
}
use uuid::Uuid;
