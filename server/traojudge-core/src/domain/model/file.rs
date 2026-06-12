use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(Uuid);

impl From<Uuid> for FileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for FileId {
    fn into(self) -> Uuid {
        self.0
    }
}
