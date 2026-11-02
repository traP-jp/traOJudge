use anyhow::Result;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use traojudge_judge_probe::queue::{Queue, definition};
use uuid::Uuid;

#[tokio::test]
#[ignore = "dedicated PostgreSQL via DATABASE_URL"]
async fn delayed_outbox_commit_and_reconnect_cannot_skip_an_event() -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let scope = Uuid::new_v4().to_string();
    let id = Uuid::new_v4();
    q.accept(id, &scope, &definition("python", "print(42)", "", "42\n")?)
        .await?;
    let mut delayed = pool.begin().await?;
    sqlx::query("INSERT INTO judge_probe_event_outbox(scope,submission,kind,payload) VALUES($1,$2,'delayed','{}')")
  .bind(&scope).bind(id).execute(&mut *delayed).await?;
    sqlx::query("INSERT INTO judge_probe_event_outbox(scope,submission,kind,payload) VALUES($1,$2,'fast','{}')")
  .bind(&scope).bind(id).execute(&pool).await?;
    let first = q.events(&scope, 0, 256).await?;
    assert_eq!(first["events"][0]["kind"], "submission.accepted");
    assert_eq!(first["events"][1]["kind"], "fast");
    assert_eq!(first["cursor"], 2);
    delayed.commit().await?;
    let resumed = q.events(&scope, 2, 256).await?;
    assert_eq!(resumed["events"][0]["kind"], "delayed");
    assert_eq!(resumed["cursor"], 3);
    // A lost delivery response can be retried; clients deduplicate by stream/cursor.
    assert_eq!(q.events(&scope, 2, 256).await?, resumed);
    assert!(q.events(&scope, 999999, 256).await.is_err());
    // Leave no runnable work for the next test on the shared disposable DB.
    for _ in 0..3 {
        q.reconcile().await?;
        let lease = q.claim("cursor-test-cleanup", 10.0).await?.unwrap();
        q.complete(&lease, json!({"verdict":"AC"})).await?;
    }
    q.reconcile().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "dedicated PostgreSQL via DATABASE_URL; test threads=1"]
async fn result_event_contains_verdict_and_replays_without_duplicate_publication() -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let scope = Uuid::new_v4().to_string();
    let id = Uuid::new_v4();
    let mut graph = definition("python", "print(42)", "", "42\n")?;
    // One checker node isolates the event/result contract from sandbox timing.
    graph.jobs = vec![(graph.jobs[2].0, json!({"phase":"check"}))];
    graph.edges.clear();
    q.accept(id, &scope, &graph).await?;
    q.reconcile().await?;
    let lease = q.claim("synthetic-checker", 10.0).await?.unwrap();
    assert_eq!(lease.id, graph.jobs[0].0);
    assert!(q.complete(&lease, json!({"verdict":"AC"})).await?);
    assert!(!q.complete(&lease, json!({"verdict":"WA"})).await?);
    q.reconcile().await?;
    q.reconcile().await?;
    let (a, b) = tokio::join!(q.events(&scope, 0, 2), q.events(&scope, 0, 2));
    assert_eq!(a?, b?);
    let mut cursor = 0;
    let mut all = Vec::new();
    loop {
        let page = q.events(&scope, cursor, 2).await?;
        let events = page["events"].as_array().unwrap();
        if events.is_empty() {
            break;
        }
        all.extend(events.iter().cloned());
        cursor = page["cursor"].as_i64().unwrap();
    }
    for (i, event) in all.iter().enumerate() {
        assert_eq!(event["cursor"], (i + 1) as i64);
    }
    let results = all
        .iter()
        .filter(|e| e["kind"] == "result.published")
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["payload"]["verdict"], "AC");
    assert_eq!(results[0]["payload"]["ordinal"], 1);
    let stamped: bool = sqlx::query_scalar(
        "SELECT published_at IS NOT NULL FROM judge_probe_submissions WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert!(stamped);
    assert_eq!(
        all.iter().filter(|e| e["kind"] == "job.finished").count(),
        1
    );
    Ok(())
}
