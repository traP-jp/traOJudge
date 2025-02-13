use std::fmt;

use sqlx::types::chrono;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub Uuid);

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

impl From<UserId> for Uuid {
    fn from(id: UserId) -> Self {
        id.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub enum UserRole {
    common_user,
    traP_user,
    admin,
}

impl From<UserRole> for i32 {
    fn from(role: UserRole) -> Self {
        match role {
            UserRole::common_user => 0,
            UserRole::traP_user => 1,
            UserRole::admin => 2,
        }
    }
}

impl UserRole {
    pub fn new(role: i32) -> anyhow::Result<Self> {
        match role {
            0 => Ok(UserRole::common_user),
            1 => Ok(UserRole::traP_user),
            2 => Ok(UserRole::admin),
            _ => anyhow::bail!("invalid role number"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub display_id: i64,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_url: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct UpdateUser {
    pub user_name: String,
    pub icon_url: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
}
