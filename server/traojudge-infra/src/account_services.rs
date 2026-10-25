use async_trait::async_trait;
use traojudge_core::{
    domain::model::account::{AccountError, Provider},
    ports::account_services::AccountExternalServices,
};

/// 設定されていない外部接続を成功扱いにしない。本番アダプターは後続で組み立てる。
pub struct UnconfiguredAccountServices;

#[async_trait]
impl AccountExternalServices for UnconfiguredAccountServices {
    fn mail_available(&self) -> bool {
        false
    }
    async fn send_email_change(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    async fn authorization_url(
        &self,
        _: Provider,
        _: &str,
        _: &str,
    ) -> Result<String, AccountError> {
        Err(AccountError::Unavailable)
    }
    async fn verify_authorization(
        &self,
        _: Provider,
        _: &str,
        _: &str,
    ) -> Result<String, AccountError> {
        Err(AccountError::Unavailable)
    }
}
