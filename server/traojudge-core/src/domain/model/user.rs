use std::{fmt, str::FromStr};

use uuid::Uuid;

/// UUIDで識別する利用者。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserId(Uuid);

impl UserId {
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self::new)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

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
    fn parses_uuid_and_normalizes_its_representation() {
        let value = "A8098C1A-F86E-4F70-917B-72F7C091480C";
        let id: UserId = value.parse().unwrap();
        assert_eq!(id.to_string(), value.to_ascii_lowercase());
        assert_eq!(UserId::new(id.as_uuid()), id);
    }

    #[test]
    fn rejects_ids_outside_resource_id_contract() {
        for id in ["", "user_abc-123", "a b", "a/b", "利用者", &"a".repeat(129)] {
            assert!(id.parse::<UserId>().is_err());
        }
    }
}
