use uuid::Uuid;

pub mod generic_dag;
pub mod single_player_dag;
pub mod two_player_dag;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericDagId(Uuid);

impl From<Uuid> for GenericDagId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for GenericDagId {
    fn into(self) -> Uuid {
        self.0
    }
}
