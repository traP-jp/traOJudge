use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use traojudge_core::domain::model::problem::{ProblemReference, ProblemVisibility};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ProblemReferenceRow {
    visibility: String,
    deleted_at: Option<DateTime<Utc>>,
}

/// 問題の削除・公開状態変更と、解説側の参照検査を直列化する。
pub(super) async fn lock_reference(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<ProblemReference>, sqlx::Error> {
    let row = sqlx::query_as::<_, ProblemReferenceRow>(
        "SELECT visibility, deleted_at FROM problems WHERE id = $1 FOR SHARE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        let visibility = match row.visibility.as_str() {
            "public" => ProblemVisibility::Public,
            "private" => ProblemVisibility::Private,
            _ => return Err(sqlx::Error::Decode("invalid problem visibility".into())),
        };
        Ok(ProblemReference {
            visibility,
            deleted_at: row.deleted_at,
        })
    })
    .transpose()
}
