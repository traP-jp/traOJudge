use async_session::chrono;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::model::user::{User, UserRole};

#[derive(Serialize, Deserialize)]
pub enum UserRoleResponse {
    #[serde(rename = "commonUser")]
    CommonUser,
    #[serde(rename = "traPUser")]
    TrapUser,
    #[serde(rename = "admin")]
    Admin,
}

impl From<UserRole> for UserRoleResponse {
    fn from(role: UserRole) -> Self {
        match role {
            UserRole::CommonUser => UserRoleResponse::CommonUser,
            UserRole::TrapUser => UserRoleResponse::TrapUser,
            UserRole::Admin => UserRoleResponse::Admin,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub display_id: i64,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_url: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
    pub role: UserRoleResponse,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse {
            id: user.id.into(),
            display_id: user.display_id,
            name: user.name,
            traq_id: user.traq_id,
            github_id: user.github_id,
            icon_url: user.icon_url,
            x_link: user.x_link,
            github_link: user.github_link,
            self_introduction: user.self_introduction,
            role: user.role.into(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct UpdateEmail {
    pub email: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePassword {
    pub old_password: String,
    pub new_password: String,
}

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMe {
    pub user_name: Option<String>,
    pub icon: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: Option<String>,
}
