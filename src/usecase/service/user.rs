use lettre::Address;

use crate::{
    domain::{
        external::mail::MailClient,
        model::{
            jwt::EmailToken,
            user::{UpdateUser, User},
        },
        repository::{auth::AuthRepository, session::SessionRepository, user::UserRepository},
    },
    presentation::context::validate::Validator,
    usecase::model::user::{UpdatePasswordData, UpdateUserData},
};

#[derive(Clone)]
pub struct UserService<UR: UserRepository, SR: SessionRepository, AR: AuthRepository, C: MailClient>
{
    user_repository: UR,
    session_repository: SR,
    auth_repository: AR,
    mail_client: C,
}

impl<UR: UserRepository, SR: SessionRepository, AR: AuthRepository, C: MailClient>
    UserService<UR, SR, AR, C>
{
    pub fn new(
        user_repository: UR,
        session_repository: SR,
        auth_repository: AR,
        mail_client: C,
    ) -> Self {
        Self {
            user_repository,
            session_repository,
            auth_repository,
            mail_client,
        }
    }
}

#[derive(Debug)]
pub enum UserError {
    ValidateError,
    Unauthorized,
    NotFound,
    InternalServerError,
}

impl<UR: UserRepository, SR: SessionRepository, AR: AuthRepository, C: MailClient>
    UserService<UR, SR, AR, C>
{
    pub async fn get_user(&self, display_id: String) -> anyhow::Result<User, UserError> {
        let display_id = display_id
            .parse::<i64>()
            .map_err(|_| UserError::ValidateError)?;

        let user = self
            .user_repository
            .get_user_by_display_id(display_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::NotFound)?;
        // todo (problems)

        Ok(user)
    }

    pub async fn get_me(&self, session_id: String) -> anyhow::Result<User, UserError> {
        let user_id = self
            .session_repository
            .get_display_id_by_session_id(&session_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::Unauthorized)?;

        let user = self
            .user_repository
            .get_user_by_display_id(user_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::NotFound)?;
        // todo (problems)

        Ok(user)
    }

    pub async fn update_me(
        &self,
        session_id: String,
        body: UpdateUserData,
    ) -> anyhow::Result<User, UserError> {
        body.validate().map_err(|_| UserError::ValidateError)?;

        let user_id = self
            .session_repository
            .get_display_id_by_session_id(&session_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::Unauthorized)?;

        let user = self
            .user_repository
            .get_user_by_display_id(user_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::InternalServerError)?;

        // todo (icon)
        let icon_url = body.icon_url.map(|icon| icon);

        self.user_repository
            .update_user(
                user_id,
                UpdateUser {
                    user_name: body.user_name.unwrap_or(user.name),
                    icon_url,
                    x_link: body.x_link.or(user.x_link),
                    github_link: body.github_link.or(user.github_link),
                    self_introduction: body.self_introduction.unwrap_or(user.self_introduction),
                },
            )
            .await
            .map_err(|_| UserError::InternalServerError)?;

        let new_user = self
            .user_repository
            .get_user_by_display_id(user_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::InternalServerError)?;
        // todo (problems)

        Ok(new_user)
    }

    pub async fn change_email(
        &self,
        session_id: String,
        email: String,
    ) -> anyhow::Result<(), UserError> {
        let user_address = email
            .parse::<Address>()
            .map_err(|_| UserError::ValidateError)?;

        let display_id = self
            .session_repository
            .get_display_id_by_session_id(&session_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::Unauthorized)?;

        if self
            .user_repository
            .is_exist_email(&email)
            .await
            .map_err(|_| UserError::InternalServerError)?
        {
            return Err(UserError::ValidateError);
        }

        let encode_key = std::env::var("ENCODE_KEY").unwrap();
        let jwt = EmailToken::encode_email_update_jwt(display_id, &email, encode_key)
            .map_err(|_| UserError::InternalServerError)?;

        // todo
        let subject = "メールアドレス変更の確認";
        let message = format!(
            "以下のリンクをクリックして、メールアドレスの変更を確認してください。
    https://link/{jwt}"
        );

        self.mail_client
            .send_mail(user_address, subject, &message)
            .await
            .map_err(|_| UserError::InternalServerError)?;

        Ok(())
    }

    pub async fn reset_password(
        &self,
        session_id: String,
        data: UpdatePasswordData,
    ) -> anyhow::Result<(), UserError> {
        data.validate().map_err(|_| UserError::ValidateError)?;

        let user_id = self
            .session_repository
            .get_user_id_by_session_id(&session_id)
            .await
            .map_err(|_| UserError::InternalServerError)?
            .ok_or(UserError::Unauthorized)?;

        match self
            .auth_repository
            .verify_user_password(user_id, &data.old_password)
            .await
        {
            Ok(true) => {
                self.auth_repository
                    .update_user_password(user_id, &data.new_password)
                    .await
                    .map_err(|_| UserError::InternalServerError)?;
                Ok(())
            }
            Ok(false) => return Err(UserError::Unauthorized),
            Err(_) => return Err(UserError::InternalServerError),
        }
    }
}
