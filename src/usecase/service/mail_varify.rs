use crate::domain::external::mail::MailClient;

pub struct MailVarifyServise<R: MailClient> {
    repository: R,
}

impl<R: MailClient> MailVarifyServise<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: MailClient> MailVarifyServise<R> {
    pub async fn send_verification_mail(
        &self,
        send_to: lettre::Address,
        jwt: &str,
    ) -> anyhow::Result<()> {
        let subject = "Verification mail";

        // todo
        let message = format!(
            "Please click the link below to verify your email address.\n\n\
            http://localhost:3000/verify?jwt={jwt}"
        );

        self.repository.send_mail(send_to, subject, &message).await
    }

    pub async fn send_change_email_verification(
        &self,
        send_to: lettre::Address,
        jwt: &str,
    ) -> anyhow::Result<()> {
        let subject = "Change email verification";

        // todo
        let message = format!(
            "Please click the link below to verify your new email address.\n\n\
            http://localhost:3000/verify?jwt={jwt}"
        );

        self.repository.send_mail(send_to, subject, &message).await
    }
}
