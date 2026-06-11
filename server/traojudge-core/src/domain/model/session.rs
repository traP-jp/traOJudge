use super::user::{UserId, UserName};

#[derive(Debug, Clone)]
pub struct SessionUser {
    pub user_id: UserId,
    pub user_name: UserName,
}
