/// 利用者の識別子。外部APIのResourceIdと同じ文字種・長さに制限する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidUserId> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(InvalidUserId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid user ID")]
pub struct InvalidUserId;

/// 誰でも閲覧できる情報。認証情報や役割は含めない。
/// 外部サービスのIDは本人申告のユーザー名で、未設定は空文字。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicUserProfile {
    pub id: UserId,
    pub display_name: String,
    pub bio: String,
    pub github_id: String,
    pub atcoder_id: String,
    pub x_id: String,
    pub revision: String,
}

#[cfg(test)]
mod tests {
    use super::UserId;

    #[test]
    fn accepts_opaque_ids_without_assuming_uuid() {
        for id in ["user_abc-123", "a", &"a".repeat(128)] {
            assert_eq!(UserId::new(id).unwrap().as_str(), id);
        }
    }

    #[test]
    fn rejects_ids_outside_resource_id_contract() {
        for id in ["", "a b", "a/b", "利用者", &"a".repeat(129)] {
            assert!(UserId::new(id).is_err());
        }
    }
}
