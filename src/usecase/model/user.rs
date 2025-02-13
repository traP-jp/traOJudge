use crate::domain::entities::user::UpdateUser;

pub struct UpdateUserRequest {
    pub user_name: String,
    pub icon_url: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
}

impl UpdateUserRequest {
    pub fn new(
        user_name: String,
        icon_url: Option<String>,
        x_link: Option<String>,
        github_link: Option<String>,
        self_introduction: String,
    ) -> Self {
        Self {
            user_name,
            icon_url,
            x_link,
            github_link,
            self_introduction,
        }
    }
}

impl From<UpdateUserRequest> for UpdateUser {
    fn from(req: UpdateUserRequest) -> Self {
        Self {
            user_name: req.user_name,
            icon_url: req.icon_url,
            x_link: req.x_link,
            github_link: req.github_link,
            self_introduction: req.self_introduction,
        }
    }
}
