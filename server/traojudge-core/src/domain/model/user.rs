use super::icon::IconId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(Uuid);

impl UserId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<Uuid> for UserId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for UserId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserName(String);

impl TryFrom<String> for UserName {
    type Error = anyhow::Error;

    fn try_from(user_name: String) -> Result<Self, Self::Error> {
        if super::rules::RuleType::UserName
            .validate(user_name.as_str())
            .is_ok()
        {
            Ok(Self(user_name))
        } else {
            anyhow::bail!("invalid user name")
        }
    }
}

impl Into<String> for UserName {
    fn into(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub user_name: UserName,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_id: Option<IconId>,
    pub x_id: Option<String>,
    pub is_system_admin: bool,
    pub self_introduction: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UpdateUser {
    pub user_name: String,
    pub icon_id: Option<IconId>,
    pub github_id: Option<String>,
    pub x_id: Option<String>,
    pub self_introduction: String,
}

/// `can_create_contest`などの拡張を想定しています
pub struct UserGlobalAuthorityAttribute {
    pub is_system_admin: bool,
    pub can_create_problem: bool,
    pub is_trap_user: bool,
}
