use super::{account::RoleName, user::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ContestError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("operation is not permitted")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("operation conflicts with current contest state")]
    Conflict,
    #[error("resource revision changed")]
    RevisionMismatch,
    #[error("invalid {field}: {reason}")]
    Validation {
        field: &'static str,
        reason: &'static str,
    },
    #[error("contest persistence operation failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}
impl ContestError {
    pub fn validation(field: &'static str, reason: &'static str) -> Self {
        Self::Validation { field, reason }
    }
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Storage(Box::new(error))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContestKind {
    Regular,
    Virtual,
}
impl ContestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Virtual => "virtual",
        }
    }
    pub fn scope_kind(self) -> &'static str {
        match self {
            Self::Regular => "contest",
            Self::Virtual => "virtual-contest",
        }
    }
    pub fn setter(self) -> RoleName {
        match self {
            Self::Regular => RoleName::ContestSetter,
            Self::Virtual => RoleName::VirtualContestSetter,
        }
    }
    pub fn participant(self) -> RoleName {
        match self {
            Self::Regular => RoleName::Contestant,
            Self::Virtual => RoleName::VirtualContestant,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipationMode {
    Individual,
    Team,
}
impl ParticipationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Individual => "individual",
            Self::Team => "team",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContestVisibility {
    Public,
    Private,
}
impl ContestVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TieBreak {
    Icpc,
    Atcoder,
}
impl TieBreak {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Icpc => "icpc",
            Self::Atcoder => "atcoder",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContestRules {
    pub tie_break: TieBreak,
    pub penalty_minutes: i32,
}
impl ContestRules {
    pub fn validate(&self) -> Result<(), ContestError> {
        if self.penalty_minutes < 0 {
            return Err(ContestError::validation(
                "rules.penaltyMinutes",
                "must be nonnegative",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Contest {
    pub id: Uuid,
    pub kind: ContestKind,
    pub created_by: Uuid,
    pub title: String,
    pub description: String,
    pub visibility: ContestVisibility,
    pub participation_mode: ParticipationMode,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub rules: ContestRules,
    pub revision: String,
    pub created_at: DateTime<Utc>,
    pub participation_epoch: Uuid,
    pub team_salt: Uuid,
}
#[derive(Serialize)]
pub struct NewContest {
    pub title: String,
    pub description: String,
    pub visibility: ContestVisibility,
    pub participation_mode: ParticipationMode,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub rules: ContestRules,
}
impl NewContest {
    pub fn validate(&self, kind: ContestKind) -> Result<(), ContestError> {
        validate_text("title", &self.title, 1, 200)?;
        validate_description(&self.description)?;
        validate_schedule(self.starts_at, self.ends_at)?;
        self.rules.validate()?;
        if kind == ContestKind::Virtual
            && (self.participation_mode != ParticipationMode::Individual
                || !self.description.is_empty())
        {
            return Err(ContestError::validation(
                "body",
                "virtual contest does not accept regular-contest settings",
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<Vec<u8>, ContestError> {
        digest(self)
    }
}
#[derive(Default)]
pub struct ContestChanges {
    pub title: Option<String>,
    pub visibility: Option<ContestVisibility>,
    pub description: Option<String>,
    pub participation_mode: Option<ParticipationMode>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub rules: Option<ContestRules>,
}
impl ContestChanges {
    pub fn validate(&self, kind: ContestKind) -> Result<(), ContestError> {
        if self.title.is_none()
            && self.visibility.is_none()
            && self.description.is_none()
            && self.participation_mode.is_none()
            && self.starts_at.is_none()
            && self.ends_at.is_none()
            && self.rules.is_none()
        {
            return Err(ContestError::validation(
                "body",
                "at least one change is required",
            ));
        }
        if let Some(title) = &self.title {
            validate_text("title", title, 1, 200)?;
        }
        if let Some(description) = &self.description {
            validate_description(description)?;
        }
        if let Some(rules) = &self.rules {
            rules.validate()?;
        }
        if kind == ContestKind::Virtual
            && (self.description.is_some() || self.participation_mode.is_some())
        {
            return Err(ContestError::validation(
                "body",
                "unsupported virtual-contest setting",
            ));
        }
        Ok(())
    }
    /// 更新前の開始時刻で判定し、開始を未来へ動かして制約を回避させない。
    pub fn validate_timing(
        &self,
        current: &Contest,
        now: DateTime<Utc>,
    ) -> Result<(), ContestError> {
        let changes_fixed_settings = self
            .title
            .as_ref()
            .is_some_and(|title| title != &current.title)
            || self
                .visibility
                .is_some_and(|visibility| visibility != current.visibility)
            || self.starts_at.is_some_and(|start| {
                start.timestamp_micros() != current.starts_at.timestamp_micros()
            })
            || self.rules.is_some_and(|rules| rules != current.rules)
            || self
                .participation_mode
                .is_some_and(|mode| mode != current.participation_mode);
        if now >= current.starts_at && changes_fixed_settings {
            return Err(ContestError::Conflict);
        }
        Ok(())
    }
}
pub fn validate_schedule(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), ContestError> {
    if start.timestamp_micros() >= end.timestamp_micros() {
        return Err(ContestError::validation("endsAt", "must be after startsAt"));
    }
    Ok(())
}
pub fn validate_text(
    field: &'static str,
    text: &str,
    min: usize,
    max: usize,
) -> Result<(), ContestError> {
    if !(min..=max).contains(&text.chars().count()) || text.contains('\0') {
        return Err(ContestError::validation(
            field,
            "must be within the character limit and contain no NUL",
        ));
    }
    Ok(())
}
fn validate_description(text: &str) -> Result<(), ContestError> {
    validate_text("description", text, 0, 200000)?;
    if pulldown_cmark::Parser::new(text).any(|event| {
        matches!(
            event,
            pulldown_cmark::Event::Html(_) | pulldown_cmark::Event::InlineHtml(_)
        )
    }) {
        return Err(ContestError::validation(
            "description",
            "raw HTML is not allowed",
        ));
    }
    Ok(())
}
pub fn digest<T: Serialize>(value: &T) -> Result<Vec<u8>, ContestError> {
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(serde_json::to_vec(value).map_err(ContestError::storage)?).to_vec())
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ContestProblem {
    pub problem_id: Uuid,
    pub label: String,
    pub order: i32,
}
#[derive(Default)]
pub struct ContestProblemInput {
    pub label: Option<String>,
    pub order: Option<i32>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Participation {
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub user_id: Uuid,
    pub display_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ContestTeam {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<TeamMember>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TeamParticipation {
    pub team: ContestTeam,
    pub joined_at: DateTime<Utc>,
    pub created: bool,
}
#[derive(Serialize, Deserialize)]
pub enum ContestReceiptResult {
    Created(Contest),
    Individual(Participation),
    Team(TeamParticipation),
}
pub struct ContestReceipt {
    pub digest: Vec<u8>,
    pub epoch: Uuid,
    pub result: ContestReceiptResult,
}

pub struct ContestPageRequest {
    pub limit: u32,
    pub after: Option<(DateTime<Utc>, Uuid)>,
}
pub struct ContestPage {
    pub items: Vec<Contest>,
    pub next: Option<(DateTime<Utc>, Uuid)>,
}

pub struct ContestScope {
    pub target: Option<(ContestKind, Uuid)>,
    pub additional_users: Vec<UserId>,
    pub lock_participants: bool,
}
impl ContestScope {
    pub fn create() -> Self {
        Self {
            target: None,
            additional_users: vec![],
            lock_participants: false,
        }
    }
    pub fn existing(kind: ContestKind, id: Uuid) -> Self {
        Self {
            target: Some((kind, id)),
            additional_users: vec![],
            lock_participants: false,
        }
    }
    pub fn with_user(mut self, user: UserId) -> Self {
        self.additional_users.push(user);
        self
    }
    pub fn with_participants(mut self) -> Self {
        self.lock_participants = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedule_requires_distinct_instants_at_persisted_precision() {
        let start = DateTime::parse_from_rfc3339("2026-11-01T00:00:00.000000001Z")
            .unwrap()
            .to_utc();
        assert!(validate_schedule(start, start + chrono::Duration::nanoseconds(1)).is_err());
        assert!(validate_schedule(start, start + chrono::Duration::microseconds(1)).is_ok());
    }
    #[test]
    fn markdown_rejects_raw_html_but_accepts_code_and_unicode() {
        assert!(validate_description("<script>alert(1)</script>").is_err());
        assert!(
            validate_description(
                "[リンク](https://example.com)\n\n`<script>`\n\n```html\n<div>本文</div>\n```"
            )
            .is_ok()
        );
        assert!(validate_text("title", &"大".repeat(200), 1, 200).is_ok());
        assert!(validate_text("title", &"大".repeat(201), 1, 200).is_err());
    }
}
