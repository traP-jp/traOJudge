//! Real PostgreSQL integration tests. Dedicated disposable DB required.
use anyhow::Result;
use serde_json::json;
use sqlx::{Row, postgres::PgPoolOptions};
use traojudge_judge_probe::queue::{Queue, definition};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL via DATABASE_URL"]
async fn receipt_replay_fencing_poison_and_ordered_publication() -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let scope = Uuid::new_v4().to_string();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let da = definition("python", "print(1)", "", "1\n")?;
    let db = definition("python", "print(2)", "", "2\n")?;
    assert_eq!(q.accept(a, &scope, &da).await?, 1);
    // Retry an ambiguous commit: same receipt and ordinal, no duplicate graph.
    assert_eq!(q.accept(a, &scope, &da).await?, 1);
    assert!(q.accept(a, &scope, &db).await.is_err());
    assert_eq!(q.accept(b, &scope, &db).await?, 2);
    // Receipt was committed before graph creation. A new controller recovers it.
    let restarted = Queue(pool.clone());
    restarted.reconcile().await?;
    // Complete B first while A's worker dies. These are independent nodes.
    let first = restarted.claim("crashed-worker", 0.04).await?.unwrap();
    let second = restarted.claim("surviving-worker", 10.0).await?.unwrap();
    assert_eq!(first.id, da.jobs[0].0);
    assert_eq!(second.id, db.jobs[0].0);
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    assert!(!restarted.complete(&first, json!({"stale":true})).await?);
    assert!(restarted.complete(&second, json!({"ok":true})).await?);
    let recovered = restarted.claim("replacement", 10.0).await?.unwrap();
    assert_eq!(recovered.id, first.id);
    assert_eq!(recovered.attempt, 2);
    assert!(
        !restarted
            .complete(&first, json!({"overwrite":true}))
            .await?
    );
    // Finish B's actual remaining DAG nodes while A is still leased.
    restarted.reconcile().await?;
    for expected in [db.jobs[1].0, db.jobs[2].0] {
        let lease = restarted.claim("surviving-worker", 10.0).await?.unwrap();
        assert_eq!(lease.id, expected);
        assert!(restarted.complete(&lease, json!({"ok":true})).await?);
        restarted.reconcile().await?;
    }
    let terminal: bool =
        sqlx::query_scalar("SELECT terminal FROM judge_probe_submissions WHERE id=$1")
            .bind(b)
            .fetch_one(&pool)
            .await?;
    assert!(terminal);
    let visible: bool =
        sqlx::query_scalar("SELECT published FROM judge_probe_submissions WHERE id=$1")
            .bind(b)
            .fetch_one(&pool)
            .await?;
    assert!(!visible);
    assert!(restarted.complete(&recovered, json!({"ok":true})).await?);
    // Finish A through queue -> engine, replaying every controller boundary.
    for _ in 0..8 {
        if let Some(lease) = restarted.claim("replacement", 10.0).await? {
            assert!(restarted.complete(&lease, json!({"ok":true})).await?);
        }
        restarted.reconcile().await?;
    }
    let visible: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM judge_probe_submissions WHERE scope=$1 AND published",
    )
    .bind(&scope)
    .fetch_one(&pool)
    .await?;
    assert_eq!(visible, 2);
    let poison = Uuid::new_v4();
    restarted
        .accept(
            poison,
            &scope,
            &definition("python", "print(3)", "", "3\n")?,
        )
        .await?;
    restarted.reconcile().await?;
    for attempt in 1..=3 {
        let lease = restarted.claim("dying-worker", 0.01).await?.unwrap();
        assert_eq!(lease.attempt, attempt);
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        restarted.reconcile().await?;
    }
    let row = sqlx::query("SELECT q.status,s.terminal,s.published FROM judge_probe_queue q
        JOIN workflow_jobs j ON j.id=q.id JOIN judge_probe_submissions s ON s.id::text=j.pipeline_id WHERE s.id=$1")
        .bind(poison).fetch_one(&pool).await?;
    assert_eq!(row.get::<String, _>("status"), "dead");
    assert!(row.get::<bool, _>("terminal") && row.get::<bool, _>("published"));
    assert!(restarted.claim("idle", 1.0).await?.is_none());
    Ok(())
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL via DATABASE_URL; run test threads=1"]
async fn five_concurrent_claimers_never_share_a_lease() -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let scope = Uuid::new_v4().to_string();
    let mut receipts = Vec::new();
    for _ in 0..5 {
        let q = q.clone();
        let scope = scope.clone();
        receipts.push(tokio::spawn(async move {
            q.accept(
                Uuid::new_v4(),
                &scope,
                &definition("python", "print(1)", "", "1\n")?,
            )
            .await
        }));
    }
    let mut ordinals = Vec::new();
    for task in receipts {
        ordinals.push(task.await??);
    }
    ordinals.sort();
    assert_eq!(ordinals, vec![1, 2, 3, 4, 5]);
    q.reconcile().await?;
    let mut claims = Vec::new();
    for i in 0..5 {
        let q = q.clone();
        claims.push(tokio::spawn(async move {
            q.claim(&format!("host-{i}"), 10.0).await
        }));
    }
    let mut ids = std::collections::HashSet::new();
    for task in claims {
        let lease = task.await??.unwrap();
        assert!(ids.insert(lease.id));
        assert!(q.complete(&lease, json!({"ok":true})).await?);
    }
    for _ in 0..16 {
        q.reconcile().await?;
        if let Some(lease) = q.claim("cleanup", 10.0).await? {
            q.complete(&lease, json!({"ok":true})).await?;
        }
    }
    q.reconcile().await?;
    let completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM judge_probe_submissions WHERE scope=$1 AND terminal AND published",
    )
    .bind(&scope)
    .fetch_one(&pool)
    .await?;
    assert_eq!(completed, 5);
    Ok(())
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL via DATABASE_URL; run test threads=1"]
async fn invalid_dag_rejected_and_completed_commit_survives_receipt_timeout() -> Result<()> {
    use traojudge_infra::postgres::workflow::PgWorkflow;
    use traojudge_workflow::{Engine, PipelineId};
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let scope = Uuid::new_v4().to_string();
    let mut bad = definition("python", "print(1)", "", "1\n")?;
    bad.edges.push((bad.jobs[2].0, bad.jobs[0].0));
    assert!(q.accept(Uuid::new_v4(), &scope, &bad).await.is_err());
    let id = Uuid::new_v4();
    let mut graph = definition("python", "print(1)", "", "1\n")?;
    graph.jobs.truncate(1);
    graph.edges.clear();
    assert_eq!(q.accept(id, &scope, &graph).await?, 1);
    assert!(q.accept(Uuid::new_v4(), &scope, &graph).await.is_err());
    q.reconcile().await?;
    let lease = q.claim("committed-result-worker", 10.0).await?.unwrap();
    q.complete(&lease, json!({"ok":true})).await?;
    Engine::new(PgWorkflow(pool.clone()), q.clone())
        .job_completed(lease.id)
        .await?;
    // Actual terminal graph commit, then crash before receipt projection.
    sqlx::query("UPDATE judge_probe_submissions SET created_at=clock_timestamp()-interval '6 minutes' WHERE id=$1")
        .bind(id).execute(&pool).await?;
    q.reconcile().await?;
    let state: String = sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id=$1")
        .bind(PipelineId::from(id).to_string())
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "completed");
    let row =
        sqlx::query("SELECT terminal,published,error FROM judge_probe_submissions WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
    assert!(row.get::<bool, _>("terminal") && row.get::<bool, _>("published"));
    assert!(row.get::<Option<String>, _>("error").is_none());
    Ok(())
}

#[tokio::test]
#[ignore = "requires dedicated PostgreSQL via DATABASE_URL; run test threads=1"]
async fn paused_reconciler_cannot_create_graph_after_receipt_terminal() -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let q = Queue(pool.clone());
    q.init().await?;
    let id = Uuid::new_v4();
    let scope = Uuid::new_v4().to_string();
    q.accept(id, &scope, &definition("python", "print(1)", "", "1\n")?)
        .await?;
    let mut lock = pool.begin().await?;
    sqlx::query("SELECT id FROM judge_probe_submissions WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *lock)
        .await?;
    let other = q.clone();
    let task = tokio::spawn(async move { other.reconcile().await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!task.is_finished());
    sqlx::query("UPDATE judge_probe_submissions SET terminal=TRUE,error='test expiry' WHERE id=$1")
        .bind(id)
        .execute(&mut *lock)
        .await?;
    lock.commit().await?;
    task.await??;
    let graph_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_pipelines WHERE id=$1)")
            .bind(id.to_string())
            .fetch_one(&pool)
            .await?;
    assert!(!graph_exists);
    assert!(q.claim("late-worker", 10.0).await?.is_none());
    Ok(())
}
