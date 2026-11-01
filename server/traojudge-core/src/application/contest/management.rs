use super::{ContestService, permissions::*};
use crate::{
    domain::model::{
        account::{AuthenticatedSession, RoleName},
        contest::*,
        request::IdempotencyKey,
    },
    ports::repository::contest::{ContestUnitOfWork, ContestUnitOfWorkProvider},
};
use uuid::Uuid;

impl<U: ContestUnitOfWorkProvider> ContestService<U> {
    pub async fn create(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        key: IdempotencyKey,
        input: NewContest,
    ) -> Result<Contest, ContestError> {
        input.validate(kind)?;
        let digest = input.digest()?;
        let mut tx = self
            .unit_of_work
            .begin(Some(actor), ContestScope::create())
            .await?;
        let role = match kind {
            ContestKind::Regular => RoleName::ServiceAdmin,
            ContestKind::Virtual => RoleName::StandardUser,
        };
        if !tx.has_service_role(actor.user_id, role).await? {
            return Err(ContestError::Forbidden);
        }
        let operation = match kind {
            ContestKind::Regular => "create-regular",
            ContestKind::Virtual => "create-virtual",
        };
        if let Some(receipt) = tx
            .receipt(actor.user_id, operation, Uuid::nil(), &key)
            .await?
        {
            check_receipt(&receipt, &digest, Uuid::nil())?;
            let ContestReceiptResult::Created(c) = receipt.result else {
                return Err(ContestError::Conflict);
            };
            if !tx.is_active(kind, c.id).await? {
                return Err(ContestError::NotFound);
            }
            tx.commit().await?;
            return Ok(c);
        }
        let c = Contest {
            id: Uuid::new_v4(),
            kind,
            created_by: actor.user_id.as_uuid(),
            title: input.title,
            description: input.description,
            visibility: input.visibility,
            participation_mode: input.participation_mode,
            starts_at: input.starts_at,
            ends_at: input.ends_at,
            rules: input.rules,
            revision: Uuid::new_v4().to_string(),
            created_at: tx.now(),
            participation_epoch: Uuid::new_v4(),
            team_salt: Uuid::new_v4(),
        };
        tx.insert(&c).await?;
        // 通常大会の管理・運営担当は手動付与。バーチャル大会だけ投稿者を運営担当にする。
        if kind == ContestKind::Virtual {
            tx.insert_role(actor.user_id, kind.setter()).await?;
        }
        let expires = self.receipt_expires_at(tx.now())?;
        tx.save_receipt(
            actor.user_id,
            operation,
            Uuid::nil(),
            &key,
            &ContestReceipt {
                digest,
                epoch: Uuid::nil(),
                result: ContestReceiptResult::Created(c.clone()),
            },
            expires,
        )
        .await?;
        tx.commit().await?;
        Ok(c)
    }
    pub async fn get(
        &self,
        actor: Option<AuthenticatedSession>,
        kind: ContestKind,
        id: Uuid,
    ) -> Result<Contest, ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(actor, ContestScope::existing(kind, id))
            .await?;
        let c = tx.current()?;
        if !can_view(&mut tx, actor, &c).await? {
            return Err(ContestError::NotFound);
        }
        tx.commit().await?;
        Ok(c)
    }
    pub async fn list(
        &self,
        actor: Option<AuthenticatedSession>,
        kind: ContestKind,
        page: ContestPageRequest,
    ) -> Result<ContestPage, ContestError> {
        if !(1..=100).contains(&page.limit) {
            return Err(ContestError::validation(
                "limit",
                "must be between 1 and 100",
            ));
        }
        let mut tx = self
            .unit_of_work
            .begin(actor, ContestScope::create())
            .await?;
        let mut items = tx.list_public(kind, &page).await?;
        let next = if items.len() > page.limit as usize {
            items.truncate(page.limit as usize);
            items.last().map(|c| (c.created_at, c.id))
        } else {
            None
        };
        tx.commit().await?;
        Ok(ContestPage { items, next })
    }
    pub async fn update(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        revision: &str,
        changes: ContestChanges,
    ) -> Result<Contest, ContestError> {
        changes.validate(kind)?;
        let mut scope = ContestScope::existing(kind, id);
        if changes.participation_mode.is_some() {
            scope = scope.with_participants();
        }
        let mut tx = self.unit_of_work.begin(Some(actor), scope).await?;
        let mut c = tx.current()?;
        require_setter(&mut tx, actor.user_id, &c).await?;
        if c.revision != revision {
            return Err(ContestError::RevisionMismatch);
        }
        changes.validate_timing(&c, tx.now())?;
        if let Some(mode) = changes.participation_mode
            && mode != c.participation_mode
        {
            tx.clear_participation().await?;
            c.participation_epoch = Uuid::new_v4();
            c.participation_mode = mode;
        }
        if let Some(title) = changes.title {
            c.title = title;
        }
        if let Some(visibility) = changes.visibility {
            c.visibility = visibility;
        }
        if let Some(description) = changes.description {
            c.description = description;
        }
        if let Some(start) = changes.starts_at {
            c.starts_at = start;
        }
        if let Some(end) = changes.ends_at {
            c.ends_at = end;
        }
        if let Some(rules) = changes.rules {
            c.rules = rules;
        }
        validate_schedule(c.starts_at, c.ends_at)?;
        c.revision = Uuid::new_v4().to_string();
        tx.save(&c).await?;
        tx.commit().await?;
        Ok(c)
    }
    pub async fn delete(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
    ) -> Result<(), ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(Some(actor), ContestScope::existing(kind, id))
            .await?;
        match kind {
            ContestKind::Regular => require_service_admin(&mut tx, actor.user_id).await?,
            ContestKind::Virtual => {
                let c = tx.current()?;
                require_setter(&mut tx, actor.user_id, &c).await?;
            }
        }
        tx.delete().await?;
        tx.commit().await
    }
}
