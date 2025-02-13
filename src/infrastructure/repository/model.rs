use sqlx::{prelude::FromRow, types::chrono, Decode, Encode, MySql, Type};
use uuid::Uuid;

#[derive(Debug, FromRow, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserIdRow(pub Uuid);

impl UserIdRow {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

impl<'a> Decode<'a, MySql> for UserIdRow {
    fn decode(
        value: <MySql as sqlx::database::HasValueRef<'a>>::ValueRef,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        <Uuid as Decode<'a, MySql>>::decode(value).map(UserIdRow)
    }
}

impl<'a> Encode<'a, MySql> for UserIdRow {
    fn encode_by_ref(
        &self,
        buf: &mut <MySql as sqlx::database::HasArguments<'a>>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull {
        self.0.encode_by_ref(buf)
    }

    fn encode(
        self,
        buf: &mut <MySql as sqlx::database::HasArguments<'a>>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull
    where
        Self: Sized,
    {
        self.0.encode(buf)
    }
}

impl Type<MySql> for UserIdRow {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <Uuid as Type<MySql>>::type_info()
    }
    fn compatible(ty: &<MySql as sqlx::Database>::TypeInfo) -> bool {
        <Uuid as Type<MySql>>::compatible(ty)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: UserIdRow,
    pub display_id: i64,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_url: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
    pub role: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
