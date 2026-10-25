use async_trait::async_trait;

use crate::domain::model::account::{AccountError, Provider};

/// 本番接続は別途実装する。トークン・認可コード・providerの応答をログへ出さない。
#[async_trait]
pub trait AccountExternalServices: Send + Sync {
    fn mail_available(&self) -> bool;
    /// 配送の受付だけを保証する。失敗時の自動再試行はせず、利用者の再送で対応する。
    async fn send_email_change(&self, email: &str, token: &str) -> Result<(), AccountError>;
    /// code_challengeはS256。state・redirect URIとproviderの対応を保持すること。
    async fn authorization_url(
        &self,
        provider: Provider,
        state: &str,
        code_challenge: &str,
    ) -> Result<String, AccountError>;
    /// 認可コードをproviderに照会し、PKCE・redirect URIを検証して安定したsubjectを返す。
    /// メールアドレスやクライアントが申告したIDをsubjectの代わりにしない。
    async fn verify_authorization(
        &self,
        provider: Provider,
        code: &str,
        code_verifier: &str,
    ) -> Result<String, AccountError>;
}
