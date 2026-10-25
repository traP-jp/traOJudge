use std::str::FromStr;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::user::{PublicUserProfile, UserId};

#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedSession {
    pub id: Uuid,
    pub user_id: UserId,
}

/// 生の秘密はログに出さない。永続化にはdigestだけを渡す。
pub struct Secret(String);

impl Secret {
    pub fn random() -> Self {
        // 独立したUUID v4を2つ使い、244bitのランダム性を持たせる。
        Self(format!(
            "{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        ))
    }

    pub fn parse(value: &str) -> Result<Self, AccountError> {
        if !(32..=256).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Err(AccountError::Unauthenticated);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> Vec<u8> {
        token_digest(self.expose())
    }
}

pub fn token_digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("resource not found")]
    NotFound,
    #[error("operation conflicts with current account state")]
    Conflict,
    #[error("resource revision changed")]
    RevisionMismatch,
    #[error("invalid {field}: {reason}")]
    Validation {
        field: &'static str,
        reason: &'static str,
    },
    #[error("external service is unavailable")]
    Unavailable,
    #[error("account storage operation failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl AccountError {
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Storage(Box::new(error))
    }

    pub fn validation(field: &'static str, reason: &'static str) -> Self {
        Self::Validation { field, reason }
    }
}

#[derive(Default)]
pub struct ProfileChanges {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub github_id: Option<String>,
    pub atcoder_id: Option<String>,
    pub x_id: Option<String>,
}

impl ProfileChanges {
    pub fn validate(&self) -> Result<(), AccountError> {
        let fields = [
            ("displayName", &self.display_name),
            ("bio", &self.bio),
            ("githubId", &self.github_id),
            ("atcoderId", &self.atcoder_id),
            ("xId", &self.x_id),
        ];
        if fields.iter().all(|(_, value)| value.is_none()) {
            return Err(AccountError::validation(
                "body",
                "at least one change is required",
            ));
        }
        for (field, value) in fields {
            if value.as_ref().is_some_and(|value| value.contains('\0')) {
                return Err(AccountError::validation(field, "NUL is not supported"));
            }
        }
        if self
            .display_name
            .as_ref()
            .is_some_and(|s| !(1..=80).contains(&s.chars().count()))
        {
            return Err(AccountError::validation(
                "displayName",
                "must contain 1 to 80 characters",
            ));
        }
        if self.bio.as_ref().is_some_and(|s| s.chars().count() > 4000) {
            return Err(AccountError::validation(
                "bio",
                "must contain at most 4000 characters",
            ));
        }
        Ok(())
    }

    pub fn apply(self, profile: &mut PublicUserProfile) {
        if let Some(value) = self.display_name {
            profile.display_name = value;
        }
        if let Some(value) = self.bio {
            profile.bio = value;
        }
        if let Some(value) = self.github_id {
            profile.github_id = value;
        }
        if let Some(value) = self.atcoder_id {
            profile.atcoder_id = value;
        }
        if let Some(value) = self.x_id {
            profile.x_id = value;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Github,
    Traq,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Github => "github",
            Self::Traq => "traq",
        }
    }
}

impl FromStr for Provider {
    type Err = AccountError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "google" => Ok(Self::Google),
            "github" => Ok(Self::Github),
            "traq" => Ok(Self::Traq),
            _ => Err(AccountError::validation("provider", "unsupported provider")),
        }
    }
}

pub struct CurrentAccount {
    pub profile: PublicUserProfile,
    pub roles: Vec<AccountRole>,
}

/// DB制約でも役割とscopeの組み合わせを保護する。
pub struct AccountRole {
    pub role: RoleName,
    pub scope: RoleScope,
    pub user_id: UserId,
    pub granted_at: DateTime<Utc>,
}

pub enum RoleScope {
    Service,
    Problem(Uuid),
    Contest(Uuid),
    VirtualContest(Uuid),
    Editorial(Uuid),
}

pub struct SignInMethod {
    pub id: Uuid,
    pub kind: SignInKind,
    pub created_at: DateTime<Utc>,
}

pub struct DeveloperToken {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct NewDeveloperToken {
    pub id: Uuid,
    pub name: String,
    pub digest: Vec<u8>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct AuthorizationAttempt {
    pub id: Uuid,
    pub session_id: Uuid,
    pub provider: Provider,
    pub state_digest: Vec<u8>,
    pub code_verifier: String,
    pub expires_at: DateTime<Utc>,
    pub completed_method: Option<Uuid>,
}

pub struct StartedAuthorization {
    pub id: Uuid,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

pub struct LinkedSignInMethod {
    pub method: SignInMethod,
    pub created: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInKind {
    Email,
    External(Provider),
}

impl SignInKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::External(provider) => provider.as_str(),
        }
    }
}

impl FromStr for SignInKind {
    type Err = InvalidAccountValue;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "email" {
            return Ok(Self::Email);
        }
        value
            .parse()
            .map(Self::External)
            .map_err(|_| InvalidAccountValue)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid persisted account value")]
pub struct InvalidAccountValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleName {
    ServiceAdmin,
    Writer,
    StandardUser,
    ProblemOwner,
    Tester,
    ContestAdmin,
    ContestSetter,
    VirtualContestSetter,
    Contestant,
    VirtualContestant,
    EditorialAuthor,
}

impl FromStr for RoleName {
    type Err = InvalidAccountValue;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "service-admin" => Ok(Self::ServiceAdmin),
            "writer" => Ok(Self::Writer),
            "standard-user" => Ok(Self::StandardUser),
            "problem-owner" => Ok(Self::ProblemOwner),
            "tester" => Ok(Self::Tester),
            "contest-admin" => Ok(Self::ContestAdmin),
            "contest-setter" => Ok(Self::ContestSetter),
            "virtual-contest-setter" => Ok(Self::VirtualContestSetter),
            "contestant" => Ok(Self::Contestant),
            "virtual-contestant" => Ok(Self::VirtualContestant),
            "editiorial-author" => Ok(Self::EditorialAuthor),
            _ => Err(InvalidAccountValue),
        }
    }
}

pub struct EmailConfirmation {
    pub email: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

impl EmailConfirmation {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), AccountError> {
        if self.consumed_at.is_some() || self.expires_at <= now {
            return Err(AccountError::validation(
                "token",
                "confirmation is invalid or expired",
            ));
        }
        Ok(())
    }
}

impl AuthorizationAttempt {
    pub fn validate(
        &self,
        actor: AuthenticatedSession,
        state_digest: &[u8],
        now: DateTime<Utc>,
    ) -> Result<(), AccountError> {
        if self.session_id != actor.id
            || self.state_digest != state_digest
            || self.expires_at <= now
        {
            return Err(AccountError::validation(
                "state",
                "authorization is invalid or expired",
            ));
        }
        Ok(())
    }
}
