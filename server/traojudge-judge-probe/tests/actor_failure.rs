//! Real child-process failure and frozen-transaction tests; no sandbox or AWS.
use anyhow::Result;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use traojudge_judge_probe::queue::{Queue, definition};
use uuid::Uuid;

struct Actor {
    child: Child,
    ready: PathBuf,
}
impl Drop for Actor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.ready);
    }
}
async fn actor(mode: &str, id: Uuid) -> Result<Actor> {
    let ready = std::env::temp_dir().join(format!("judge-actor-ready-{}", Uuid::new_v4()));
    let child = Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", "child_actor", "--nocapture"])
        .env("JUDGE_TEST_ACTOR", mode)
        .env("JUDGE_TEST_RECEIPT", id.to_string())
        .env("JUDGE_TEST_READY", &ready)
        .stdout(Stdio::null())
        .spawn()?;
    let actor = Actor { child, ready };
    let start = Instant::now();
    while !actor.ready.exists() {
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "child did not become ready"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(actor)
}
async fn queue() -> Result<Queue> {
    let q = Queue(
        PgPoolOptions::new()
            .max_connections(12)
            .connect(&std::env::var("DATABASE_URL")?)
            .await?,
    );
    q.init().await?;
    Ok(q)
}
async fn receipt(q: &Queue) -> Result<(Uuid, traojudge_workflow::JobId)> {
    let id = Uuid::new_v4();
    let mut graph = definition("python", "print(42)", "", "42\n")?;
    graph.jobs = vec![(graph.jobs[2].0, json!({"phase":"check"}))];
    graph.edges.clear();
    q.accept(id, &id.to_string(), &graph).await?;
    Ok((id, graph.jobs[0].0))
}

#[tokio::test]
#[ignore = "child helper; invoked by actor tests"]
async fn child_actor() -> Result<()> {
    let Ok(mode) = std::env::var("JUDGE_TEST_ACTOR") else {
        return Ok(());
    };
    let q = queue().await?;
    if mode == "claim" {
        q.claim("killed-process", 0.3)
            .await?
            .expect("runnable fixture");
        std::fs::write(std::env::var("JUDGE_TEST_READY")?, "ready")?;
        tokio::time::sleep(Duration::from_secs(60)).await;
    } else {
        let mut tx = q.0.begin().await?;
        sqlx::query("SET LOCAL idle_in_transaction_session_timeout='300ms'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM judge_probe_submissions WHERE id=$1 FOR UPDATE")
            .bind(Uuid::parse_str(&std::env::var("JUDGE_TEST_RECEIPT")?)?)
            .fetch_one(&mut *tx)
            .await?;
        std::fs::write(std::env::var("JUDGE_TEST_READY")?, "ready")?;
        tokio::time::sleep(Duration::from_secs(60)).await;
        tx.rollback().await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "dedicated PostgreSQL via DATABASE_URL; threads=1"]
async fn killed_process_leaves_a_durable_lease_that_another_process_can_recover() -> Result<()> {
    let q = queue().await?;
    let (id, job) = receipt(&q).await?;
    q.reconcile().await?;
    let mut child = actor("claim", id).await?;
    child.child.kill()?;
    assert!(!child.child.wait()?.success());
    tokio::time::sleep(Duration::from_millis(350)).await;
    let recovered = q.claim("replacement-process", 5.0).await?.unwrap();
    assert_eq!(recovered.id, job);
    assert_eq!(recovered.attempt, 2);
    assert!(q.complete(&recovered, json!({"verdict":"AC"})).await?);
    q.reconcile().await?;
    let published: bool =
        sqlx::query_scalar("SELECT published FROM judge_probe_submissions WHERE id=$1")
            .bind(id)
            .fetch_one(&q.0)
            .await?;
    assert!(published);
    Ok(())
}

#[tokio::test]
#[ignore = "dedicated PostgreSQL via DATABASE_URL; threads=1"]
async fn server_timeout_releases_a_row_lock_even_when_the_client_process_is_frozen() -> Result<()> {
    let q = queue().await?;
    let (id, job) = receipt(&q).await?;
    let child = actor("freeze", id).await?;
    assert!(
        Command::new("kill")
            .args(["-STOP", &child.child.id().to_string()])
            .status()?
            .success()
    );
    let start = Instant::now();
    q.reconcile().await?;
    assert!(start.elapsed() < Duration::from_secs(3));
    let lease = q.claim("after-frozen-client", 5.0).await?.unwrap();
    assert_eq!(lease.id, job);
    q.complete(&lease, json!({"verdict":"AC"})).await?;
    q.reconcile().await?;
    Ok(())
}
