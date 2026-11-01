/// 作成・参加要求の再送を識別するキー。HTTPや対象機能の型には依存しない。
pub struct IdempotencyKey(String);
#[derive(Debug, thiserror::Error)]
#[error("idempotency key must contain 1 to 128 ASCII letters, digits, '.', '_', ':' or '-'")]
pub struct InvalidIdempotencyKey;
impl IdempotencyKey {
    pub fn parse(value: &str) -> Result<Self, InvalidIdempotencyKey> {
        if !(1..=128).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
        {
            return Err(InvalidIdempotencyKey);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
