use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use traojudge_core::{
    domain::model::{
        account::{AccountRole, AuthenticatedSession, RoleName, RoleScope},
        editorial::*,
        problem::ProblemReference,
        user::UserId,
    },
    ports::repository::editorial::{EditorialUnitOfWork, EditorialUnitOfWorkProvider},
};
use uuid::Uuid;

pub struct PgEditorialUnitOfWorkProvider {
    pool: PgPool,
}
impl PgEditorialUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub struct PgEditorialUnitOfWork {
    tx: Transaction<'static, Postgres>,
    now: DateTime<Utc>,
}
#[async_trait]
impl EditorialUnitOfWorkProvider for PgEditorialUnitOfWorkProvider {
    type UnitOfWork<'a> = PgEditorialUnitOfWork;
    async fn begin(
        &self,
        actor: Option<AuthenticatedSession>,
        affected_users: &[UserId],
    ) -> Result<Self::UnitOfWork<'_>, EditorialError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut users = affected_users
            .iter()
            .map(UserId::as_uuid)
            .collect::<Vec<_>>();
        if let Some(actor) = actor {
            users.push(actor.user_id.as_uuid());
        }
        users.sort_unstable();
        users.dedup();
        let locked: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE id = ANY($1) ORDER BY id FOR UPDATE")
                .bind(&users)
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?;
        if let Some(actor) = actor {
            if !locked.contains(&actor.user_id.as_uuid()) {
                return Err(EditorialError::Unauthenticated);
            }
            let valid: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM web_sessions WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > clock_timestamp())")
                .bind(actor.id).bind(actor.user_id.as_uuid()).fetch_one(&mut *tx).await.map_err(storage)?;
            if !valid {
                return Err(EditorialError::Unauthenticated);
            }
        }
        if locked.len() != users.len() {
            return Err(EditorialError::NotFound);
        }
        let now = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        Ok(PgEditorialUnitOfWork { tx, now })
    }
}
fn storage(error: sqlx::Error) -> EditorialError {
    EditorialError::storage(error)
}

#[derive(sqlx::FromRow)]
struct EditorialRow {
    id: Uuid,
    problem_id: Uuid,
    author_id: Uuid,
    title: String,
    body: String,
    visibility: String,
    revision: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
#[derive(sqlx::FromRow)]
struct CreationReceiptRow {
    request_digest: Vec<u8>,
    #[sqlx(flatten)]
    editorial: EditorialRow,
}

impl TryFrom<EditorialRow> for Editorial {
    type Error = EditorialError;
    fn try_from(row: EditorialRow) -> Result<Self, Self::Error> {
        let visibility = match row.visibility.as_str() {
            "public" => EditorialVisibility::Public,
            "private" => EditorialVisibility::Private,
            _ => {
                return Err(EditorialError::storage(std::io::Error::other(
                    "invalid editorial visibility",
                )));
            }
        };
        Ok(Self {
            id: row.id,
            problem_id: row.problem_id,
            author_id: UserId::new(row.author_id),
            title: row.title,
            body: row.body,
            visibility,
            revision: row.revision,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}
fn visibility(value: EditorialVisibility) -> &'static str {
    match value {
        EditorialVisibility::Public => "public",
        EditorialVisibility::Private => "private",
    }
}
fn assignment(editorial_id: Uuid, user_id: UserId, granted_at: DateTime<Utc>) -> AccountRole {
    AccountRole {
        role: RoleName::EditorialAuthor,
        scope: RoleScope::Editorial(editorial_id),
        user_id,
        granted_at,
    }
}

#[async_trait]
impl EditorialUnitOfWork for PgEditorialUnitOfWork {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    async fn has_service_role(
        &mut self,
        user_id: UserId,
        role: RoleName,
    ) -> Result<bool, EditorialError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_roles WHERE user_id = $1 AND role = $2 AND scope_kind = 'service' AND scope_id IS NULL)")
            .bind(user_id.as_uuid()).bind(role.as_str()).fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn problem_reference(
        &mut self,
        id: Uuid,
    ) -> Result<Option<ProblemReference>, EditorialError> {
        super::problems::lock_reference(&mut self.tx, id)
            .await
            .map_err(storage)
    }
    async fn has_problem_role(
        &mut self,
        user_id: UserId,
        problem_id: Uuid,
        role: RoleName,
    ) -> Result<bool, EditorialError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_roles WHERE user_id = $1 AND scope_kind = 'problem' AND scope_id = $2 AND role = $3)")
            .bind(user_id.as_uuid()).bind(problem_id).bind(role.as_str())
            .fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn find(&mut self, id: Uuid) -> Result<Option<Editorial>, EditorialError> {
        // 親を先にロックし、問題削除（親→解説）と逆順にロックを取らない。
        let parent: Option<Uuid> = sqlx::query_scalar(
            "SELECT problem_id FROM editorials WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?;
        let Some(parent) = parent else {
            return Ok(None);
        };
        if super::problems::lock_reference(&mut self.tx, parent)
            .await
            .map_err(storage)?
            .is_none_or(|p| p.deleted_at.is_some())
        {
            return Ok(None);
        }
        sqlx::query_as::<_,EditorialRow>("SELECT id,problem_id,author_id,title,body,visibility,revision,created_at,updated_at FROM editorials WHERE id = $1 AND deleted_at IS NULL FOR UPDATE")
            .bind(id).fetch_optional(&mut *self.tx).await.map_err(storage)?.map(TryInto::try_into).transpose()
    }
    async fn list_public(
        &mut self,
        problem_id: Uuid,
        page: &EditorialPageRequest,
    ) -> Result<Vec<Editorial>, EditorialError> {
        let rows=sqlx::query_as::<_,EditorialRow>("SELECT id,problem_id,author_id,title,body,visibility,revision,created_at,updated_at FROM editorials WHERE problem_id = $1 AND deleted_at IS NULL AND visibility = 'public' AND ($2::timestamptz IS NULL OR (created_at,id) > ($2,$3)) ORDER BY created_at,id LIMIT $4")
            .bind(problem_id).bind(page.cursor.as_ref().map(|cursor|cursor.created_at)).bind(page.cursor.as_ref().map(|cursor|cursor.id))
            .bind(i64::from(page.limit)+1).fetch_all(&mut *self.tx).await.map_err(storage)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
    async fn insert(&mut self, p: &Editorial) -> Result<(), EditorialError> {
        sqlx::query("INSERT INTO editorials (id,problem_id,author_id,title,body,visibility,revision,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(p.id).bind(p.problem_id).bind(p.author_id.as_uuid()).bind(&p.title).bind(&p.body).bind(visibility(p.visibility)).bind(&p.revision).bind(p.created_at).bind(p.updated_at)
            .execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn save(&mut self, p: &Editorial) -> Result<(), EditorialError> {
        // 親・投稿者・公開範囲・作成日時は本文更新では変更しない。
        sqlx::query("UPDATE editorials SET title = $2,body = $3,revision = $4,updated_at = $5 WHERE id = $1")
            .bind(p.id).bind(&p.title).bind(&p.body).bind(&p.revision).bind(p.updated_at).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn save_publication(&mut self, p: &Editorial) -> Result<(), EditorialError> {
        sqlx::query("UPDATE editorials SET visibility = $2, revision = $3, updated_at = $4 WHERE id = $1 AND deleted_at IS NULL")
            .bind(p.id).bind(visibility(p.visibility)).bind(&p.revision).bind(p.updated_at)
            .execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn delete(&mut self, id: Uuid) -> Result<(), EditorialError> {
        sqlx::query("UPDATE editorials SET deleted_at = clock_timestamp(), revision = gen_random_uuid()::text WHERE id = $1 AND deleted_at IS NULL")
            .bind(id).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn author_assignment(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<Option<AccountRole>, EditorialError> {
        let granted_at:Option<DateTime<Utc>>=sqlx::query_scalar("SELECT granted_at FROM account_roles WHERE role = 'editiorial-author' AND scope_kind = 'editorial' AND scope_id = $1 AND user_id = $2")
            .bind(editorial_id).bind(user_id.as_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        Ok(granted_at.map(|time| assignment(editorial_id, user_id, time)))
    }
    async fn insert_author(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<AccountRole, EditorialError> {
        let granted_at=sqlx::query_scalar("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,'editiorial-author','editorial',$2) RETURNING granted_at")
            .bind(user_id.as_uuid()).bind(editorial_id).fetch_one(&mut *self.tx).await.map_err(storage)?;
        Ok(assignment(editorial_id, user_id, granted_at))
    }
    async fn delete_author(
        &mut self,
        editorial_id: Uuid,
        user_id: UserId,
    ) -> Result<(), EditorialError> {
        sqlx::query("DELETE FROM account_roles WHERE role = 'editiorial-author' AND scope_kind = 'editorial' AND scope_id = $1 AND user_id = $2")
            .bind(editorial_id).bind(user_id.as_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn creation_receipt(
        &mut self,
        user_id: UserId,
        key: &EditorialCreationKey,
    ) -> Result<Option<EditorialCreationReceipt>, EditorialError> {
        let row = sqlx::query_as::<_, CreationReceiptRow>(
            "SELECT request_digest, editorial_id AS id, problem_id, user_id AS author_id,
                    title, body, 'private' AS visibility, revision, created_at, created_at AS updated_at
             FROM editorial_creation_receipts
             WHERE user_id = $1 AND key = $2 AND expires_at > $3"
        ).bind(user_id.as_uuid()).bind(key.as_str()).bind(self.now)
            .fetch_optional(&mut *self.tx).await.map_err(storage)?;
        row.map(|row| {
            Ok(EditorialCreationReceipt {
                request_digest: row.request_digest,
                editorial: row.editorial.try_into()?,
            })
        })
        .transpose()
    }
    async fn save_creation_receipt(
        &mut self,
        key: &EditorialCreationKey,
        receipt: &EditorialCreationReceipt,
        expires_at: DateTime<Utc>,
    ) -> Result<(), EditorialError> {
        let p = &receipt.editorial;
        sqlx::query("INSERT INTO editorial_creation_receipts (user_id,key,request_digest,editorial_id,problem_id,title,body,revision,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (user_id,key) DO UPDATE SET request_digest = EXCLUDED.request_digest,editorial_id = EXCLUDED.editorial_id,problem_id = EXCLUDED.problem_id,title = EXCLUDED.title,body = EXCLUDED.body,revision = EXCLUDED.revision,created_at = EXCLUDED.created_at,expires_at = EXCLUDED.expires_at")
            .bind(p.author_id.as_uuid()).bind(key.as_str()).bind(&receipt.request_digest).bind(p.id).bind(p.problem_id).bind(&p.title).bind(&p.body).bind(&p.revision).bind(p.created_at).bind(expires_at)
            .execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn commit(self) -> Result<(), EditorialError> {
        self.tx.commit().await.map_err(storage)
    }
    async fn rollback(self) -> Result<(), EditorialError> {
        self.tx.rollback().await.map_err(storage)
    }
}
