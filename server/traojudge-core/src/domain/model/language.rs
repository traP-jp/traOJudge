#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageId(i64);

impl From<i64> for LanguageId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<LanguageId> for i64 {
    fn from(id: LanguageId) -> Self {
        id.0
    }
}

#[derive(Clone)]
pub struct Language {
    pub id: LanguageId,
    pub name: String,
}
