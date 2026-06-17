use chrono::DateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(i64);

impl UserId {
    pub fn new(id: i64) -> Self {
        Self(id)
    }
}

impl From<i64> for UserId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<UserId> for i64 {
    fn from(id: UserId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_object_key: Option<String>,
    pub x_id: Option<String>,
    pub self_introduction: String,
    pub is_system_admin: bool,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

pub struct UpdateUser {
    pub user_name: String,
    pub icon_object_key: Option<String>,
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
