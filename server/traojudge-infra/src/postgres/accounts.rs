use super::users::UserProfileRow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use traojudge_core::{
    domain::model::{
        account::*,
        user::{PublicUserProfile, UserId},
    },
    ports::repository::account::{AccountUnitOfWork, AccountUnitOfWorkProvider},
};
use uuid::Uuid;

pub struct PgAccountUnitOfWorkProvider {
    pool: PgPool,
}
impl PgAccountUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub struct PgAccountUnitOfWork {
    tx: Transaction<'static, Postgres>,
    actor: AuthenticatedSession,
    now: DateTime<Utc>,
}

#[async_trait]
impl AccountUnitOfWorkProvider for PgAccountUnitOfWorkProvider {
    type UnitOfWork<'a> = PgAccountUnitOfWork;
    async fn begin(
        &self,
        actor: AuthenticatedSession,
    ) -> Result<Self::UnitOfWork<'_>, AccountError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let user = sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
            .bind(actor.user_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        if user.is_none() {
            return Err(AccountError::Unauthenticated);
        }
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM web_sessions WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > clock_timestamp())")
            .bind(actor.id).bind(actor.user_id.as_uuid()).fetch_one(&mut *tx).await.map_err(storage)?;
        if !valid {
            return Err(AccountError::Unauthenticated);
        }
        let now = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        Ok(PgAccountUnitOfWork { tx, actor, now })
    }
}

fn storage(error: sqlx::Error) -> AccountError {
    AccountError::storage(error)
}

fn write_error(error: sqlx::Error) -> AccountError {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        AccountError::Conflict
    } else {
        storage(error)
    }
}

async fn current(
    tx: &mut Transaction<'_, Postgres>,
    user_id: UserId,
) -> Result<CurrentAccount, AccountError> {
    let row = sqlx::query_as::<_, UserProfileRow>("SELECT id, display_name, bio, github_id, atcoder_id, x_id, revision FROM users WHERE id = $1")
        .bind(user_id.as_uuid())
        .fetch_one(&mut **tx)
        .await
        .map_err(storage)?;
    let rows = sqlx::query("SELECT role, scope_kind, scope_id, granted_at FROM account_roles WHERE user_id = $1 ORDER BY role, scope_kind, scope_id")
        .bind(user_id.as_uuid()).fetch_all(&mut **tx).await.map_err(storage)?;
    let roles = rows
        .into_iter()
        .map(|row| -> Result<AccountRole, sqlx::Error> {
            let kind: &str = row.try_get("scope_kind")?;
            let scope = match kind {
                "service" => RoleScope::Service,
                "problem" => RoleScope::Problem(row.try_get("scope_id")?),
                "contest" => RoleScope::Contest(row.try_get("scope_id")?),
                "virtual-contest" => RoleScope::VirtualContest(row.try_get("scope_id")?),
                "editorial" => RoleScope::Editorial(row.try_get("scope_id")?),
                _ => return Err(sqlx::Error::Decode("invalid role scope".into())),
            };
            Ok(AccountRole {
                role: row
                    .try_get::<&str, _>("role")?
                    .parse()
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                scope,
                user_id,
                granted_at: row.try_get("granted_at")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    Ok(CurrentAccount {
        profile: row.into(),
        roles,
    })
}

fn method(row: PgRow) -> Result<SignInMethod, sqlx::Error> {
    Ok(SignInMethod {
        id: row.try_get("id")?,
        kind: row
            .try_get::<&str, _>("kind")?
            .parse()
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
        created_at: row.try_get("created_at")?,
    })
}

fn developer_token(row: PgRow) -> Result<DeveloperToken, sqlx::Error> {
    Ok(DeveloperToken {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        created_at: row.try_get("created_at")?,
        expires_at: row.try_get("expires_at")?,
        revoked_at: row.try_get("revoked_at")?,
    })
}

fn attempt(row: PgRow) -> Result<AuthorizationAttempt, AccountError> {
    let provider: &str = row.try_get("provider").map_err(storage)?;
    Ok(AuthorizationAttempt {
        id: row.try_get("id").map_err(storage)?,
        session_id: row.try_get("session_id").map_err(storage)?,
        provider: provider.parse()?,
        state_digest: row.try_get("state_digest").map_err(storage)?,
        code_verifier: row.try_get("code_verifier").map_err(storage)?,
        expires_at: row.try_get("expires_at").map_err(storage)?,
        completed_method: row.try_get("completed_method").map_err(storage)?,
    })
}

#[async_trait]
impl AccountUnitOfWork for PgAccountUnitOfWork {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    async fn current_account(&mut self) -> Result<CurrentAccount, AccountError> {
        current(&mut self.tx, self.actor.user_id).await
    }
    async fn save_profile(&mut self, p: &PublicUserProfile) -> Result<(), AccountError> {
        sqlx::query("UPDATE users SET display_name = $2, bio = $3, github_id = $4, atcoder_id = $5, x_id = $6, revision = $7 WHERE id = $1")
            .bind(self.actor.user_id.as_uuid()).bind(&p.display_name).bind(&p.bio).bind(&p.github_id).bind(&p.atcoder_id).bind(&p.x_id).bind(&p.revision)
            .execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn sign_in_methods(&mut self) -> Result<Vec<SignInMethod>, AccountError> {
        let rows = sqlx::query("SELECT id, kind, created_at FROM sign_in_methods WHERE user_id = $1 ORDER BY created_at, id")
            .bind(self.actor.user_id.as_uuid()).fetch_all(&mut *self.tx).await.map_err(storage)?;
        rows.into_iter()
            .map(method)
            .collect::<Result<_, _>>()
            .map_err(storage)
    }
    async fn delete_sign_in_method(&mut self, id: Uuid) -> Result<(), AccountError> {
        sqlx::query("DELETE FROM sign_in_methods WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(self.actor.user_id.as_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn developer_tokens(&mut self) -> Result<Vec<DeveloperToken>, AccountError> {
        let rows = sqlx::query("SELECT id, name, created_at, expires_at, revoked_at FROM developer_tokens WHERE user_id = $1 ORDER BY created_at, id")
            .bind(self.actor.user_id.as_uuid()).fetch_all(&mut *self.tx).await.map_err(storage)?;
        rows.into_iter()
            .map(developer_token)
            .collect::<Result<_, _>>()
            .map_err(storage)
    }
    async fn insert_developer_token(
        &mut self,
        token: NewDeveloperToken,
    ) -> Result<DeveloperToken, AccountError> {
        let row = sqlx::query("INSERT INTO developer_tokens (id, user_id, name, token_digest, expires_at) VALUES ($1, $2, $3, $4, $5) RETURNING id, name, created_at, expires_at, revoked_at")
            .bind(token.id).bind(self.actor.user_id.as_uuid()).bind(token.name).bind(token.digest).bind(token.expires_at)
            .fetch_one(&mut *self.tx).await.map_err(write_error)?;
        developer_token(row).map_err(storage)
    }
    async fn revoke_developer_token(&mut self, id: Uuid) -> Result<bool, AccountError> {
        let result = sqlx::query("UPDATE developer_tokens SET revoked_at = COALESCE(revoked_at, clock_timestamp()) WHERE id = $1 AND user_id = $2")
            .bind(id).bind(self.actor.user_id.as_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(result.rows_affected() != 0)
    }
    async fn email_owner(&mut self, email: &str) -> Result<Option<UserId>, AccountError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM sign_in_methods WHERE kind = 'email' AND subject = $1",
        )
        .bind(email)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(id.map(UserId::new))
    }
    async fn replace_email_confirmation(
        &mut self,
        email: &str,
        digest: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), AccountError> {
        sqlx::query("UPDATE email_changes SET consumed_at = clock_timestamp() WHERE user_id = $1 AND consumed_at IS NULL")
            .bind(self.actor.user_id.as_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO email_changes (token_digest, user_id, email, expires_at) VALUES ($1, $2, $3, $4)")
            .bind(digest).bind(self.actor.user_id.as_uuid()).bind(email).bind(expires_at).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn email_confirmation(
        &mut self,
        digest: &[u8],
    ) -> Result<Option<EmailConfirmation>, AccountError> {
        let row = sqlx::query("SELECT email, expires_at, consumed_at FROM email_changes WHERE token_digest = $1 AND user_id = $2")
            .bind(digest).bind(self.actor.user_id.as_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        row.map(|row| -> Result<_, sqlx::Error> {
            Ok(EmailConfirmation {
                email: row.try_get("email")?,
                expires_at: row.try_get("expires_at")?,
                consumed_at: row.try_get("consumed_at")?,
            })
        })
        .transpose()
        .map_err(storage)
    }
    async fn consume_email_confirmation(&mut self, digest: &[u8]) -> Result<(), AccountError> {
        sqlx::query("UPDATE email_changes SET consumed_at = clock_timestamp() WHERE token_digest = $1 AND user_id = $2")
            .bind(digest).bind(self.actor.user_id.as_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn set_email(&mut self, email: &str) -> Result<(), AccountError> {
        sqlx::query("INSERT INTO sign_in_methods (user_id, kind, subject) VALUES ($1, 'email', $2) ON CONFLICT (user_id, kind) DO UPDATE SET subject = EXCLUDED.subject")
            .bind(self.actor.user_id.as_uuid()).bind(email).execute(&mut *self.tx).await.map_err(write_error)?;
        Ok(())
    }
    async fn revoke_other_sessions(&mut self) -> Result<(), AccountError> {
        sqlx::query("UPDATE web_sessions SET revoked_at = clock_timestamp() WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL")
            .bind(self.actor.user_id.as_uuid()).bind(self.actor.id).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn insert_authorization(
        &mut self,
        attempt: AuthorizationAttempt,
    ) -> Result<(), AccountError> {
        sqlx::query("INSERT INTO account_authorizations (id, user_id, session_id, provider, state_digest, code_verifier, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(attempt.id).bind(self.actor.user_id.as_uuid()).bind(self.actor.id).bind(attempt.provider.as_str()).bind(attempt.state_digest).bind(attempt.code_verifier).bind(attempt.expires_at)
            .execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn authorization(
        &mut self,
        id: Uuid,
    ) -> Result<Option<AuthorizationAttempt>, AccountError> {
        let row = sqlx::query("SELECT id, session_id, provider, state_digest, code_verifier, expires_at, completed_method FROM account_authorizations WHERE id = $1 AND user_id = $2")
            .bind(id).bind(self.actor.user_id.as_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        row.map(attempt).transpose()
    }
    async fn external_identity(
        &mut self,
        provider: Provider,
        subject: &str,
    ) -> Result<Option<(UserId, SignInMethod)>, AccountError> {
        let row = sqlx::query("SELECT id, kind, created_at, user_id FROM sign_in_methods WHERE kind = $1 AND subject = $2")
            .bind(provider.as_str()).bind(subject).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        row.map(|row| -> Result<_, sqlx::Error> {
            Ok((UserId::new(row.try_get("user_id")?), method(row)?))
        })
        .transpose()
        .map_err(storage)
    }
    async fn insert_external_method(
        &mut self,
        provider: Provider,
        subject: &str,
    ) -> Result<SignInMethod, AccountError> {
        let row = sqlx::query("INSERT INTO sign_in_methods (user_id, kind, subject) VALUES ($1, $2, $3) RETURNING id, kind, created_at")
            .bind(self.actor.user_id.as_uuid()).bind(provider.as_str()).bind(subject).fetch_one(&mut *self.tx).await.map_err(write_error)?;
        method(row).map_err(storage)
    }
    async fn finish_authorization(
        &mut self,
        id: Uuid,
        method_id: Uuid,
    ) -> Result<(), AccountError> {
        sqlx::query("UPDATE account_authorizations SET completed_method = $2, code_verifier = '' WHERE id = $1 AND user_id = $3")
            .bind(id).bind(method_id).bind(self.actor.user_id.as_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn commit(self) -> Result<(), AccountError> {
        self.tx.commit().await.map_err(storage)
    }
    async fn rollback(self) -> Result<(), AccountError> {
        self.tx.rollback().await.map_err(storage)
    }
}
