use crate::{
    domain::model::{
        account::{AccountRole, AuthenticatedSession, RoleName},
        editorial::*,
        problem::ProblemVisibility,
        user::UserId,
    },
    ports::repository::editorial::{EditorialUnitOfWork, EditorialUnitOfWorkProvider},
};
use chrono::Duration;
use uuid::Uuid;

pub struct EditorialService<U> {
    unit_of_work: U,
    creation_retention: Duration,
}
impl<U: EditorialUnitOfWorkProvider> EditorialService<U> {
    pub fn new(unit_of_work: U, creation_retention: Duration) -> Result<Self, EditorialError> {
        if creation_retention <= Duration::zero() {
            return Err(EditorialError::validation(
                "creation_retention",
                "must be positive",
            ));
        }
        Ok(Self {
            unit_of_work,
            creation_retention,
        })
    }
    pub async fn create(
        &self,
        actor: AuthenticatedSession,
        key: EditorialCreationKey,
        input: NewEditorial,
    ) -> Result<Editorial, EditorialError> {
        input.validate()?;
        let digest = input.digest();
        let mut tx = self.unit_of_work.begin(Some(actor), &[]).await?;
        if !tx
            .has_service_role(actor.user_id, RoleName::StandardUser)
            .await?
        {
            return Err(EditorialError::Forbidden);
        }
        require_readable_problem(&mut tx, input.problem_id, Some(actor.user_id)).await?;
        if let Some(receipt) = tx.creation_receipt(actor.user_id, &key).await? {
            if receipt.request_digest != digest {
                return Err(EditorialError::Conflict);
            }
            // 削除済み解説の作成要求を再送しても、復活や過去の本文の再公開はしない。
            tx.find(receipt.editorial.id)
                .await?
                .ok_or(EditorialError::NotFound)?;
            tx.commit().await?;
            return Ok(receipt.editorial);
        }
        let editorial = Editorial {
            id: Uuid::new_v4(),
            problem_id: input.problem_id,
            author_id: actor.user_id,
            title: input.title,
            body: input.body,
            // TODO(editorial-policy): 非公開＋投稿者への編集権限付与は暫定合意。仕様確定時に見直す。
            visibility: EditorialVisibility::Private,
            revision: Uuid::new_v4().to_string(),
            created_at: tx.now(),
            updated_at: tx.now(),
        };
        tx.insert(&editorial).await?;
        tx.insert_author(editorial.id, actor.user_id).await?;
        let expires_at = tx
            .now()
            .checked_add_signed(self.creation_retention)
            .ok_or_else(|| {
                EditorialError::validation("creation_retention", "duration is too large")
            })?;
        tx.save_creation_receipt(
            &key,
            &EditorialCreationReceipt {
                request_digest: digest,
                editorial: editorial.clone(),
            },
            expires_at,
        )
        .await?;
        tx.commit().await?;
        Ok(editorial)
    }
    pub async fn get(
        &self,
        actor: Option<AuthenticatedSession>,
        id: Uuid,
    ) -> Result<Editorial, EditorialError> {
        let mut tx = self.unit_of_work.begin(actor, &[]).await?;
        let editorial = tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        if editorial.visibility != EditorialVisibility::Public {
            let permitted = if let Some(actor) = actor {
                tx.author_assignment(id, actor.user_id).await?.is_some()
            } else {
                false
            };
            if !permitted {
                return Err(EditorialError::NotFound);
            }
        }
        require_readable_problem(
            &mut tx,
            editorial.problem_id,
            actor.map(|actor| actor.user_id),
        )
        .await?;
        tx.commit().await?;
        Ok(editorial)
    }
    pub async fn list(
        &self,
        actor: Option<AuthenticatedSession>,
        problem_id: Uuid,
        page: EditorialPageRequest,
    ) -> Result<EditorialPage, EditorialError> {
        page.validate(problem_id)?;
        let mut tx = self.unit_of_work.begin(actor, &[]).await?;
        require_readable_problem(&mut tx, problem_id, actor.map(|actor| actor.user_id)).await?;
        let mut items = tx.list_public(problem_id, &page).await?;
        let next_cursor = if items.len() > page.limit as usize {
            items.truncate(page.limit as usize);
            items.last().map(|item| EditorialCursor {
                problem_id,
                created_at: item.created_at,
                id: item.id,
            })
        } else {
            None
        };
        tx.commit().await?;
        Ok(EditorialPage { items, next_cursor })
    }
    pub async fn update(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        revision: &str,
        changes: EditorialChanges,
    ) -> Result<Editorial, EditorialError> {
        changes.validate()?;
        let mut tx = self.unit_of_work.begin(Some(actor), &[]).await?;
        let mut editorial = tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        require_readable_problem(&mut tx, editorial.problem_id, Some(actor.user_id)).await?;
        if tx.author_assignment(id, actor.user_id).await?.is_none() {
            return Err(EditorialError::Forbidden);
        }
        // problem_idは更新対象に含めない。別の問題への付け替えはできない。
        if editorial.revision != revision {
            return Err(EditorialError::RevisionMismatch);
        }
        changes.apply(&mut editorial);
        editorial.revision = Uuid::new_v4().to_string();
        editorial.updated_at = tx.now();
        tx.save(&editorial).await?;
        tx.commit().await?;
        Ok(editorial)
    }
    pub async fn delete(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
    ) -> Result<(), EditorialError> {
        let mut tx = self.unit_of_work.begin(Some(actor), &[]).await?;
        let editorial = tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        require_owner_or_admin(&mut tx, actor, &editorial).await?;
        tx.delete(id).await?;
        tx.commit().await
    }

    pub async fn publish(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        revision: &str,
    ) -> Result<Editorial, EditorialError> {
        self.change_publication(actor, id, Some(revision), EditorialVisibility::Public)
            .await
    }

    pub async fn unpublish(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
    ) -> Result<(), EditorialError> {
        self.change_publication(actor, id, None, EditorialVisibility::Private)
            .await?;
        Ok(())
    }

    async fn change_publication(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        revision: Option<&str>,
        visibility: EditorialVisibility,
    ) -> Result<Editorial, EditorialError> {
        let mut tx = self.unit_of_work.begin(Some(actor), &[]).await?;
        let mut editorial = tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        require_owner_or_admin(&mut tx, actor, &editorial).await?;
        if revision.is_some_and(|revision| revision != editorial.revision) {
            return Err(EditorialError::RevisionMismatch);
        }
        if editorial.visibility != visibility {
            editorial.visibility = visibility;
            editorial.revision = Uuid::new_v4().to_string();
            editorial.updated_at = tx.now();
            tx.save_publication(&editorial).await?;
        }
        tx.commit().await?;
        Ok(editorial)
    }

    pub async fn grant_author(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        user_id: UserId,
    ) -> Result<(AccountRole, bool), EditorialError> {
        let mut tx = self.unit_of_work.begin(Some(actor), &[user_id]).await?;
        if !tx
            .has_service_role(actor.user_id, RoleName::ServiceAdmin)
            .await?
        {
            return Err(EditorialError::Forbidden);
        }
        tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        let (assignment, changed) = match tx.author_assignment(id, user_id).await? {
            Some(assignment) => (assignment, false),
            None => (tx.insert_author(id, user_id).await?, true),
        };
        tx.commit().await?;
        Ok((assignment, changed))
    }
    pub async fn revoke_author(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        user_id: UserId,
    ) -> Result<(), EditorialError> {
        let mut tx = self.unit_of_work.begin(Some(actor), &[user_id]).await?;
        if !tx
            .has_service_role(actor.user_id, RoleName::ServiceAdmin)
            .await?
        {
            return Err(EditorialError::Forbidden);
        }
        tx.find(id).await?.ok_or(EditorialError::NotFound)?;
        tx.delete_author(id, user_id).await?;
        tx.commit().await
    }
}

/// 投稿者の識別と編集担当の役割は分ける。担当を任されても削除・公開変更の権限は増えない。
async fn require_owner_or_admin<T: EditorialUnitOfWork>(
    tx: &mut T,
    actor: AuthenticatedSession,
    editorial: &Editorial,
) -> Result<(), EditorialError> {
    if editorial.author_id == actor.user_id
        || tx
            .has_service_role(actor.user_id, RoleName::ServiceAdmin)
            .await?
    {
        Ok(())
    } else {
        Err(EditorialError::Forbidden)
    }
}

async fn require_readable_problem<T: EditorialUnitOfWork>(
    tx: &mut T,
    id: Uuid,
    viewer: Option<UserId>,
) -> Result<(), EditorialError> {
    let problem = tx
        .problem_reference(id)
        .await?
        .ok_or(EditorialError::NotFound)?;
    if problem.deleted_at.is_some() {
        return Err(EditorialError::NotFound);
    }
    if problem.visibility == ProblemVisibility::Public {
        return Ok(());
    }
    if let Some(viewer) = viewer {
        for role in [RoleName::ProblemOwner, RoleName::Tester] {
            if tx.has_problem_role(viewer, id, role).await? {
                return Ok(());
            }
        }
        // TODO(contests): 通常大会と問題の関連付けができたら、その大会のcontest-setterも検査する。
    }
    Err(EditorialError::NotFound)
}
