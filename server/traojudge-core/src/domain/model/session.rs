use super::user::UserId;

#[derive(Debug, Clone)]
pub struct SessionUser {
    pub user_id: UserId,
}
