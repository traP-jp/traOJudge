use async_trait::async_trait;
use chrono::DateTime;
use sqlx::{Connection, FromRow, mysql::MySqlConnection};
use traojudge_core::domain::{
    model::user::{UpdateUser, User, UserGlobalAuthorityAttribute, UserId},
    repository::user::UserRepository,
};

#[derive(Debug)]
pub struct MariaDbUserRepository<'a> {
    connection: &'a mut MySqlConnection,
}

impl<'a> MariaDbUserRepository<'a> {
    pub fn new(connection: &'a mut MySqlConnection) -> Self {
        Self { connection }
    }
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    name: String,
    icon_object_key: Option<String>,
    profile_traq_id: Option<String>,
    profile_github_id: Option<String>,
    x_id: Option<String>,
    self_introduction: String,
    is_system_admin: bool,
    created_at: DateTime<chrono::Utc>,
    updated_at: DateTime<chrono::Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: UserId::from(row.id),
            name: row.name,
            traq_id: row.profile_traq_id,
            github_id: row.profile_github_id,
            icon_object_key: row.icon_object_key,
            x_id: row.x_id,
            self_introduction: row.self_introduction,
            is_system_admin: row.is_system_admin,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct AuthorityRow {
    is_system_admin: bool,
    can_create_problem: bool,
    is_trap_user: bool,
}

impl From<AuthorityRow> for UserGlobalAuthorityAttribute {
    fn from(row: AuthorityRow) -> Self {
        Self {
            is_system_admin: row.is_system_admin,
            can_create_problem: row.can_create_problem,
            is_trap_user: row.is_trap_user,
        }
    }
}

#[async_trait]
impl UserRepository for MariaDbUserRepository<'_> {
    async fn get_user_by_id(&mut self, id: UserId) -> anyhow::Result<Option<User>> {
        sqlx::query_as::<_, UserRow>(
            r#"
SELECT
    u.id,
    u.name,
    u.icon_object_key,
    u.profile_traq_id,
    u.profile_github_id,
    u.x_id,
    u.self_introduction,
    COALESCE(a.is_system_admin, FALSE) AS is_system_admin,
    u.created_at,
    u.updated_at
FROM users u
LEFT JOIN user_global_authority_attributes a ON a.user_id = u.id
WHERE u.id = ?
"#,
        )
        .bind(i64::from(id))
        .fetch_optional(&mut *self.connection)
        .await
        .map(|row| row.map(User::from))
        .map_err(Into::into)
    }

    async fn get_user_by_name(&mut self, name: &str) -> anyhow::Result<Option<User>> {
        sqlx::query_as::<_, UserRow>(
            r#"
SELECT
    u.id,
    u.name,
    u.icon_object_key,
    u.profile_traq_id,
    u.profile_github_id,
    u.x_id,
    u.self_introduction,
    COALESCE(a.is_system_admin, FALSE) AS is_system_admin,
    u.created_at,
    u.updated_at
FROM users u
LEFT JOIN user_global_authority_attributes a ON a.user_id = u.id
WHERE u.name = ?
"#,
        )
        .bind(name)
        .fetch_optional(&mut *self.connection)
        .await
        .map(|row| row.map(User::from))
        .map_err(Into::into)
    }

    async fn create_user(&mut self, name: &str) -> anyhow::Result<UserId> {
        let mut transaction = self.connection.begin().await?;
        let result = sqlx::query(
            r#"
INSERT INTO users (name)
VALUES (?)
"#,
        )
        .bind(name)
        .execute(transaction.as_mut())
        .await?;
        let user_id = UserId::from(result.last_insert_id() as i64);

        sqlx::query(
            r#"
INSERT INTO user_global_authority_attributes
    (user_id, is_system_admin, can_create_problem, is_trap_user)
VALUES (?, FALSE, FALSE, FALSE)
"#,
        )
        .bind(i64::from(user_id))
        .execute(transaction.as_mut())
        .await?;

        transaction.commit().await?;
        Ok(user_id)
    }

    async fn update_user(&mut self, id: UserId, user: UpdateUser) -> anyhow::Result<()> {
        let id = i64::from(id);
        let result = sqlx::query(
            r#"
UPDATE users
SET
    name = ?,
    icon_object_key = ?,
    profile_github_id = ?,
    x_id = ?,
    self_introduction = ?
WHERE id = ?
"#,
        )
        .bind(user.user_name)
        .bind(user.icon_object_key)
        .bind(user.github_id)
        .bind(user.x_id)
        .bind(user.self_introduction)
        .bind(id)
        .execute(&mut *self.connection)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("user not found: {id}");
        }

        Ok(())
    }

    async fn get_user_global_authority_attribute(
        &mut self,
        id: UserId,
    ) -> anyhow::Result<Option<UserGlobalAuthorityAttribute>> {
        sqlx::query_as::<_, AuthorityRow>(
            r#"
SELECT is_system_admin, can_create_problem, is_trap_user
FROM user_global_authority_attributes
WHERE user_id = ?
"#,
        )
        .bind(i64::from(id))
        .fetch_optional(&mut *self.connection)
        .await
        .map(|row| row.map(UserGlobalAuthorityAttribute::from))
        .map_err(Into::into)
    }

    async fn update_user_global_authority_attribute(
        &mut self,
        id: UserId,
        attribute: UserGlobalAuthorityAttribute,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO user_global_authority_attributes
    (user_id, is_system_admin, can_create_problem, is_trap_user)
VALUES (?, ?, ?, ?)
ON DUPLICATE KEY UPDATE
    is_system_admin = VALUES(is_system_admin),
    can_create_problem = VALUES(can_create_problem),
    is_trap_user = VALUES(is_trap_user)
"#,
        )
        .bind(i64::from(id))
        .bind(attribute.is_system_admin)
        .bind(attribute.can_create_problem)
        .bind(attribute.is_trap_user)
        .execute(&mut *self.connection)
        .await?;

        Ok(())
    }
}
