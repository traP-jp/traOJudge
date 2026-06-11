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

#[derive(Debug, Clone, PartialEq)]
pub enum UserRole {
    CommonUser,
    TrapUser,
    Admin,
}

impl From<UserRole> for i32 {
    fn from(role: UserRole) -> Self {
        match role {
            UserRole::CommonUser => 0,
            UserRole::TrapUser => 1,
            UserRole::Admin => 2,
        }
    }
}

impl UserRole {
    pub fn new(role: i32) -> anyhow::Result<Self> {
        match role {
            0 => Ok(UserRole::CommonUser),
            1 => Ok(UserRole::TrapUser),
            2 => Ok(UserRole::Admin),
            _ => anyhow::bail!("invalid role number"),
        }
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
    pub self_introduction: String,
    pub role: UserRole,
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
