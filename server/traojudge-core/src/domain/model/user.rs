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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserDisplayId(Uuid);

impl From<Uuid> for UserDisplayId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for UserDisplayId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub display_id: UserDisplayId,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_id: Option<IconId>,
    pub x_id: Option<String>,
    pub self_introduction: String,
    pub is_system_admin: bool,
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
