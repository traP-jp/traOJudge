use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use email_address::EmailAddress;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    domain::model::account::*,
    ports::{
        account_services::AccountExternalServices,
        repository::account::{AccountUnitOfWork, AccountUnitOfWorkProvider},
    },
};

pub struct AccountService<R, E> {
    unit_of_work: R,
    external: E,
}

impl<R: AccountUnitOfWorkProvider, E: AccountExternalServices> AccountService<R, E> {
    pub fn new(unit_of_work: R, external: E) -> Self {
        Self {
            unit_of_work,
            external,
        }
    }

    pub async fn current_account(
        &self,
        actor: AuthenticatedSession,
    ) -> Result<CurrentAccount, AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        let account = tx.current_account().await?;
        tx.commit().await?;
        Ok(account)
    }

    pub async fn update_profile(
        &self,
        actor: AuthenticatedSession,
        revision: &str,
        changes: ProfileChanges,
    ) -> Result<CurrentAccount, AccountError> {
        changes.validate()?;
        let mut tx = self.unit_of_work.begin(actor).await?;
        let mut account = tx.current_account().await?;
        if account.profile.revision != revision {
            return Err(AccountError::RevisionMismatch);
        }
        changes.apply(&mut account.profile);
        account.profile.revision = Uuid::new_v4().to_string();
        tx.save_profile(&account.profile).await?;
        tx.commit().await?;
        Ok(account)
    }

    pub async fn sign_in_methods(
        &self,
        actor: AuthenticatedSession,
    ) -> Result<Vec<SignInMethod>, AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        let methods = tx.sign_in_methods().await?;
        tx.commit().await?;
        Ok(methods)
    }

    pub async fn remove_sign_in_method(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
    ) -> Result<(), AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        let methods = tx.sign_in_methods().await?;
        let method = methods
            .iter()
            .find(|method| method.id == id)
            .ok_or(AccountError::NotFound)?;
        if method.kind == SignInKind::Email {
            return Err(AccountError::NotFound);
        }
        if methods.len() <= 1 {
            return Err(AccountError::Conflict);
        }
        tx.delete_sign_in_method(id).await?;
        tx.commit().await
    }

    pub async fn developer_tokens(
        &self,
        actor: AuthenticatedSession,
    ) -> Result<Vec<DeveloperToken>, AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        let tokens = tx.developer_tokens().await?;
        tx.commit().await?;
        Ok(tokens)
    }

    pub async fn issue_developer_token(
        &self,
        actor: AuthenticatedSession,
        name: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(DeveloperToken, Secret), AccountError> {
        if !(1..=100).contains(&name.chars().count()) || name.contains('\0') {
            return Err(AccountError::validation(
                "name",
                "must contain 1 to 100 characters without NUL",
            ));
        }
        let mut tx = self.unit_of_work.begin(actor).await?;
        if expires_at.is_some_and(|time| time <= tx.now()) {
            return Err(AccountError::validation(
                "expiresAt",
                "must be in the future",
            ));
        }
        let secret = Secret::random();
        let token = tx
            .insert_developer_token(NewDeveloperToken {
                id: Uuid::new_v4(),
                name,
                expires_at,
                digest: secret.digest(),
            })
            .await?;
        tx.commit().await?;
        Ok((token, secret))
    }

    pub async fn revoke_developer_token(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
    ) -> Result<(), AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        if !tx.revoke_developer_token(id).await? {
            return Err(AccountError::NotFound);
        }
        tx.commit().await
    }

    pub async fn request_email_change(
        &self,
        actor: AuthenticatedSession,
        email: &str,
    ) -> Result<(), AccountError> {
        if email.chars().count() > 254 || email.parse::<EmailAddress>().is_err() {
            return Err(AccountError::validation("email", "invalid email address"));
        }
        // 登録状況を調べる前に可用性を検査し、未設定時の応答も統一する。
        if !self.external.mail_available() {
            return Err(AccountError::Unavailable);
        }
        let mut tx = self.unit_of_work.begin(actor).await?;
        if tx
            .email_owner(email)
            .await?
            .is_some_and(|owner| owner != actor.user_id)
        {
            return tx.commit().await;
        }
        let secret = Secret::random();
        tx.replace_email_confirmation(email, &secret.digest(), tx.now() + Duration::minutes(30))
            .await?;
        tx.commit().await?;
        // 外部I/O中にDBロックを保持しない。配送失敗で登録状況を開示せず、自動再試行もしない。
        let _ = self
            .external
            .send_email_change(email, secret.expose())
            .await;
        Ok(())
    }

    pub async fn complete_email_change(
        &self,
        actor: AuthenticatedSession,
        token: &str,
    ) -> Result<(), AccountError> {
        let mut tx = self.unit_of_work.begin(actor).await?;
        let digest = token_digest(token);
        let confirmation = tx.email_confirmation(&digest).await?.ok_or_else(|| {
            AccountError::validation("token", "confirmation is invalid or expired")
        })?;
        confirmation.validate(tx.now())?;
        tx.set_email(&confirmation.email).await?;
        tx.consume_email_confirmation(&digest).await?;
        tx.revoke_other_sessions().await?;
        tx.commit().await
    }

    pub async fn start_authorization(
        &self,
        actor: AuthenticatedSession,
        provider: Provider,
    ) -> Result<StartedAuthorization, AccountError> {
        ensure_provider_available(&self.sign_in_methods(actor).await?, provider)?;
        let state = Secret::random();
        let verifier = Secret::random();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.expose().as_bytes()));
        let url = self
            .external
            .authorization_url(provider, state.expose(), &challenge)
            .await?;
        let mut tx = self.unit_of_work.begin(actor).await?;
        ensure_provider_available(&tx.sign_in_methods().await?, provider)?;
        let id = Uuid::new_v4();
        let expires_at = tx.now() + Duration::minutes(10);
        tx.insert_authorization(AuthorizationAttempt {
            id,
            session_id: actor.id,
            provider,
            state_digest: state.digest(),
            code_verifier: verifier.expose().to_owned(),
            expires_at,
            completed_method: None,
        })
        .await?;
        tx.commit().await?;
        Ok(StartedAuthorization {
            id,
            url,
            expires_at,
        })
    }

    pub async fn complete_authorization(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        code: &str,
        state: &str,
    ) -> Result<LinkedSignInMethod, AccountError> {
        if code.is_empty() || state.is_empty() {
            return Err(AccountError::validation(
                "body",
                "code and state are required",
            ));
        }
        let digest = token_digest(state);
        let mut tx = self.unit_of_work.begin(actor).await?;
        let attempt = validated_attempt(&mut tx, actor, id, &digest).await?;
        if let Some(method_id) = attempt.completed_method {
            let method = completed_method(&mut tx, method_id).await?;
            tx.commit().await?;
            return Ok(LinkedSignInMethod {
                method,
                created: false,
            });
        }
        tx.commit().await?;
        let subject = self
            .external
            .verify_authorization(attempt.provider, code, &attempt.code_verifier)
            .await?;
        if subject.is_empty() || subject.len() > 512 || subject.contains('\0') {
            return Err(AccountError::Unavailable);
        }
        // 外部照会中の失効・削除・別の追加を考慮し、確定直前にも検査する。
        let mut tx = self.unit_of_work.begin(actor).await?;
        let attempt = validated_attempt(&mut tx, actor, id, &digest).await?;
        if let Some(method_id) = attempt.completed_method {
            let method = completed_method(&mut tx, method_id).await?;
            tx.commit().await?;
            return Ok(LinkedSignInMethod {
                method,
                created: false,
            });
        }
        let (method, created) = match tx.external_identity(attempt.provider, &subject).await? {
            Some((owner, method)) if owner == actor.user_id => (method, false),
            Some(_) => return Err(AccountError::Conflict),
            None => {
                ensure_provider_available(&tx.sign_in_methods().await?, attempt.provider)?;
                (
                    tx.insert_external_method(attempt.provider, &subject)
                        .await?,
                    true,
                )
            }
        };
        tx.finish_authorization(id, method.id).await?;
        tx.commit().await?;
        Ok(LinkedSignInMethod { method, created })
    }
}

fn ensure_provider_available(
    methods: &[SignInMethod],
    provider: Provider,
) -> Result<(), AccountError> {
    if methods
        .iter()
        .any(|method| method.kind == SignInKind::External(provider))
    {
        return Err(AccountError::Conflict);
    }
    Ok(())
}

async fn validated_attempt<T: AccountUnitOfWork>(
    tx: &mut T,
    actor: AuthenticatedSession,
    id: Uuid,
    digest: &[u8],
) -> Result<AuthorizationAttempt, AccountError> {
    let attempt = tx
        .authorization(id)
        .await?
        .ok_or_else(|| AccountError::validation("state", "authorization is invalid or expired"))?;
    attempt.validate(actor, digest, tx.now())?;
    Ok(attempt)
}

async fn completed_method<T: AccountUnitOfWork>(
    tx: &mut T,
    id: Uuid,
) -> Result<SignInMethod, AccountError> {
    tx.sign_in_methods()
        .await?
        .into_iter()
        .find(|method| method.id == id)
        .ok_or_else(|| AccountError::validation("state", "completed method was removed"))
}
