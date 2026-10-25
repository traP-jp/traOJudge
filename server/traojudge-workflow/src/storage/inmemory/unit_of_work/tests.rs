use std::time::Duration;

use super::*;

async fn provider_with_table() -> Result<InMemoryUnitOfWorkProvider, sqlx::Error> {
    let provider = InMemoryUnitOfWorkProvider::new().await?;
    let mut work = provider.begin().await?;
    sqlx::query("CREATE TABLE entries (value INTEGER NOT NULL)")
        .execute(&mut *work.transaction)
        .await?;
    work.commit().await?;
    Ok(provider)
}

async fn entry_count(provider: &InMemoryUnitOfWorkProvider) -> Result<i64, sqlx::Error> {
    let mut work = provider.begin().await?;
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&mut *work.transaction)
        .await?;
    work.rollback().await?;
    Ok(count)
}

#[tokio::test]
async fn commit_persists_changes() -> Result<(), sqlx::Error> {
    let provider = provider_with_table().await?;
    let mut work = provider.begin().await?;
    sqlx::query("INSERT INTO entries (value) VALUES (42)")
        .execute(&mut *work.transaction)
        .await?;
    work.commit().await?;

    assert_eq!(entry_count(&provider).await?, 1);
    Ok(())
}

#[tokio::test]
async fn rollback_discards_changes() -> Result<(), sqlx::Error> {
    let provider = provider_with_table().await?;
    let mut work = provider.begin().await?;
    sqlx::query("INSERT INTO entries (value) VALUES (42)")
        .execute(&mut *work.transaction)
        .await?;
    work.rollback().await?;

    assert_eq!(entry_count(&provider).await?, 0);
    Ok(())
}

#[tokio::test]
async fn drop_discards_changes() -> Result<(), sqlx::Error> {
    let provider = provider_with_table().await?;
    let mut work = provider.begin().await?;
    sqlx::query("INSERT INTO entries (value) VALUES (42)")
        .execute(&mut *work.transaction)
        .await?;
    drop(work);

    assert_eq!(entry_count(&provider).await?, 0);
    Ok(())
}

#[tokio::test]
async fn new_providers_have_independent_databases() -> Result<(), sqlx::Error> {
    let first = provider_with_table().await?;
    let second = InMemoryUnitOfWorkProvider::new().await?;
    let mut work = second.begin().await?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'entries'")
            .fetch_one(&mut *work.transaction)
            .await?;
    work.rollback().await?;

    assert_eq!(count, 0);
    assert_eq!(entry_count(&first).await?, 0);
    Ok(())
}

#[tokio::test]
async fn clones_share_the_database() -> Result<(), sqlx::Error> {
    let provider = provider_with_table().await?;
    let clone = provider.clone();
    let mut work = provider.begin().await?;
    sqlx::query("INSERT INTO entries (value) VALUES (42)")
        .execute(&mut *work.transaction)
        .await?;
    work.commit().await?;
    drop(provider);

    assert_eq!(entry_count(&clone).await?, 1);
    Ok(())
}

#[tokio::test]
async fn begin_waits_for_active_work_and_cancelled_waiters_preserve_data() -> Result<(), sqlx::Error>
{
    let provider = provider_with_table().await?;
    let mut work = provider.begin().await?;
    sqlx::query("INSERT INTO entries (value) VALUES (42)")
        .execute(&mut *work.transaction)
        .await?;

    assert!(
        tokio::time::timeout(Duration::from_millis(10), provider.begin())
            .await
            .is_err()
    );
    work.commit().await?;

    assert_eq!(entry_count(&provider).await?, 1);
    Ok(())
}
