use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use traojudge_infra::postgres::workflow::PgWorkflow;
use traojudge_workflow::{
    Engine, JobId, JobStartCallback, PipelineId,
    payload::{Payload, Revision0},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Queue(pub PgPool);
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub jobs: Vec<(JobId, Value)>,
    pub edges: Vec<(JobId, JobId)>,
}
#[derive(Debug)]
pub struct Lease {
    pub id: JobId,
    pub token: Uuid,
    pub attempt: i32,
    pub payload: Value,
}

#[async_trait]
impl JobStartCallback for Queue {
    type Input = Revision0<Value>;
    type Error = sqlx::Error;
    async fn start(&self, id: JobId, payload: Payload<Self::Input>) -> Result<(), Self::Error> {
        // The callback is at-least-once. A single durable queue entry per node
        // survives both a callback retry and a controller crash after this commit.
        sqlx::query(
            "INSERT INTO judge_probe_queue(id,payload) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(id.to_string())
        .bind(payload.into_value())
        .execute(&self.0)
        .await?;
        Ok(())
    }
}

impl Queue {
    pub async fn init(&self) -> Result<()> {
        traojudge_infra::postgres::MIGRATOR.run(&self.0).await?;
        let mut tx = self.0.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(44001)")
            .execute(&mut *tx)
            .await?;
        let guarded:bool=sqlx::query_scalar("SELECT coalesce(rolconfig @> ARRAY['idle_in_transaction_session_timeout=30s','idle_session_timeout=90s','lock_timeout=5s','statement_timeout=20s'],false) FROM pg_roles WHERE rolname=current_user")
            .fetch_one(&mut *tx).await?;
        if !guarded {
            sqlx::raw_sql(include_str!("db-guards.sql"))
                .execute(&mut *tx)
                .await?;
        }
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&mut *tx)
            .await?;
        let events_installed: bool =
            sqlx::query_scalar("SELECT to_regclass('judge_probe_events') IS NOT NULL")
                .fetch_one(&mut *tx)
                .await?;
        if !events_installed {
            sqlx::raw_sql(include_str!("events.sql"))
                .execute(&mut *tx)
                .await?;
        }
        let stamped: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='probe_stamp_publication')",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !stamped {
            sqlx::raw_sql(include_str!("publication.sql"))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn events(&self, scope: &str, after: i64, limit: i64) -> Result<Value> {
        ensure!(
            after >= 0 && (1..=256).contains(&limit),
            "invalid event cursor or page size"
        );
        let mut tx = self.0.begin().await?;
        sqlx::query("SELECT judge_probe_dispatch_events(256)")
            .execute(&mut *tx)
            .await?;
        let head: Option<i64> =
            sqlx::query_scalar("SELECT cursor FROM judge_probe_event_streams WHERE scope=$1")
                .bind(scope)
                .fetch_optional(&mut *tx)
                .await?;
        ensure!(
            after <= head.unwrap_or(0),
            "cursor ahead of stream; resync required"
        );
        let rows=sqlx::query("SELECT cursor,submission,kind,payload FROM judge_probe_events WHERE scope=$1 AND cursor>$2 ORDER BY cursor LIMIT $3")
            .bind(scope).bind(after).bind(limit).fetch_all(&mut *tx).await?;
        let cursor = rows
            .last()
            .map(|r| r.get::<i64, _>("cursor"))
            .unwrap_or(after);
        let events = rows
            .iter()
            .map(|r| {
                json!({"cursor":r.get::<i64,_>("cursor"),"submission":r.get::<Uuid,_>("submission"),
            "kind":r.get::<String,_>("kind"),"payload":r.get::<Value,_>("payload")})
            })
            .collect::<Vec<_>>();
        tx.commit().await?;
        Ok(json!({"cursor":cursor,"events":events}))
    }
    /// Receipt ordering is the database commit order within a scope. A retry
    /// uses the same UUID and definition, even when the prior commit is unknown.
    pub async fn accept(&self, id: Uuid, scope: &str, definition: &Definition) -> Result<i64> {
        ensure!(!scope.is_empty() && scope.len() <= 128, "invalid scope");
        ensure!(
            !definition.jobs.is_empty() && definition.jobs.len() <= 64,
            "invalid graph size"
        );
        validate(definition)?;
        let value = serde_json::to_value(definition)?;
        ensure!(value.to_string().len() <= 262144, "definition too large");
        let mut tx = self.0.begin().await?;
        sqlx::query("INSERT INTO judge_probe_order(scope) VALUES($1) ON CONFLICT DO NOTHING")
            .bind(scope)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT scope FROM judge_probe_order WHERE scope=$1 FOR UPDATE")
            .bind(scope)
            .fetch_one(&mut *tx)
            .await?;
        if let Some(row) =
            sqlx::query("SELECT scope,ordinal,definition FROM judge_probe_submissions WHERE id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
        {
            ensure!(
                row.get::<String, _>("scope") == scope
                    && row.get::<Value, _>("definition") == value,
                "idempotency key reused with a different submission"
            );
            return Ok(row.get("ordinal"));
        }
        let ordinal: i64 = sqlx::query_scalar(
            "UPDATE judge_probe_order SET accepted=accepted+1 WHERE scope=$1 RETURNING accepted",
        )
        .bind(scope)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO judge_probe_submissions(id,scope,ordinal,definition) VALUES($1,$2,$3,$4)",
        )
        .bind(id)
        .bind(scope)
        .bind(ordinal)
        .bind(value)
        .execute(&mut *tx)
        .await?;
        for (job, _) in &definition.jobs {
            let inserted = sqlx::query("INSERT INTO judge_probe_reserved_jobs(id,submission)
                SELECT $1,$2 WHERE NOT EXISTS(SELECT 1 FROM workflow_jobs WHERE id=$1) ON CONFLICT DO NOTHING")
                .bind(job.to_string()).bind(id).execute(&mut *tx).await?.rows_affected();
            ensure!(
                inserted == 1,
                "job ID already belongs to another submission"
            );
        }
        tx.commit().await?;
        Ok(ordinal)
    }
    /// Every operation is replayable. Receipt -> graph and graph -> queue are
    /// separate commits: this reconciler closes both crash windows using the
    /// original immutable graph, rather than assuming dispatch succeeded.
    pub async fn reconcile(&self) -> Result<()> {
        let engine = Engine::new(PgWorkflow(self.0.clone()), self.clone());
        // A hard receipt deadline also bounds permanent controller/definition
        // errors that happen before the first queue entry can be created.
        let expired: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM judge_probe_submissions
            WHERE NOT terminal AND created_at < clock_timestamp()-interval '5 minutes'",
        )
        .fetch_all(&self.0)
        .await?;
        for id in expired {
            let mut tx = self.0.begin().await?;
            let current: Option<bool> = sqlx::query_scalar(
                "SELECT terminal FROM judge_probe_submissions WHERE id=$1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if current != Some(false) {
                continue;
            }
            let state: Option<String> =
                sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id=$1 FOR UPDATE")
                    .bind(id.to_string())
                    .fetch_optional(&mut *tx)
                    .await?;
            let error = if state.as_deref() == Some("completed") {
                None
            } else {
                Some("receipt recovery deadline exceeded")
            };
            sqlx::query(
                "UPDATE workflow_pipelines SET state='cancelled' WHERE id=$1 AND state='running'",
            )
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE judge_probe_submissions SET terminal=TRUE,finished_at=clock_timestamp(),error=$2 WHERE id=$1")
                .bind(id).bind(error).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        let receipts = sqlx::query("SELECT id,definition FROM judge_probe_submissions WHERE NOT terminal ORDER BY reconciled_at NULLS FIRST,id LIMIT 256")
            .fetch_all(&self.0).await?;
        for row in receipts {
            let id: Uuid = row.get("id");
            // Same lock order as expiry: receipt, then Engine's pipeline row.
            // This prevents a paused reconciler creating a graph after its DLQ.
            let mut receipt_tx = self.0.begin().await?;
            let active: bool = sqlx::query_scalar(
                "SELECT NOT terminal FROM judge_probe_submissions WHERE id=$1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(&mut *receipt_tx)
            .await?;
            if !active {
                continue;
            }
            let definition: Definition = serde_json::from_value(row.get("definition"))?;
            let jobs = definition
                .jobs
                .into_iter()
                .map(|(id, v)| Ok((id, Payload::new(v)?)))
                .collect::<Result<Vec<_>>>()?;
            // Replay also recovers Ready/Starting nodes left between engine commits.
            if let Err(error) = engine
                .start_pipeline(PipelineId::from(id), jobs, &definition.edges)
                .await
            {
                // Other receipts, committed results and DLQ processing must still advance.
                eprintln!("pipeline recovery failed for {id}: {error}");
            }
            sqlx::query(
                "UPDATE judge_probe_submissions SET reconciled_at=clock_timestamp() WHERE id=$1",
            )
            .bind(id)
            .execute(&mut *receipt_tx)
            .await?;
            receipt_tx.commit().await?;
        }
        // Exhausted leases have an explicit terminal destination in PostgreSQL.
        sqlx::query(
            "UPDATE judge_probe_queue SET status='dead',deadline=NULL,
            result=jsonb_build_object('infrastructure_error','lease attempts exhausted')
            WHERE status='leased' AND deadline <= clock_timestamp() AND attempt=3",
        )
        .execute(&self.0)
        .await?;
        let results = sqlx::query("SELECT id,status FROM judge_probe_queue WHERE status IN ('done','dead') AND NOT applied ORDER BY id LIMIT 256")
            .fetch_all(&self.0).await?;
        for row in results {
            let id: String = row.get("id");
            let job = JobId::from(Uuid::parse_str(&id)?);
            let applied = if row.get::<String, _>("status") == "dead" {
                engine.job_failed(job).await.map_err(anyhow::Error::from)
            } else {
                engine.job_completed(job).await.map_err(anyhow::Error::from)
            };
            if let Err(error) = applied {
                eprintln!("result recovery failed for {id}: {error}");
                continue;
            }
            sqlx::query("UPDATE judge_probe_queue SET applied=TRUE WHERE id=$1")
                .bind(id)
                .execute(&self.0)
                .await?;
        }
        sqlx::query("UPDATE judge_probe_submissions s SET terminal=TRUE,finished_at=clock_timestamp()
            FROM workflow_pipelines p WHERE s.id::text=p.id AND p.state IN ('completed','cancelled') AND NOT s.terminal")
            .execute(&self.0).await?;
        // Publication advances a contiguous terminal prefix. Later work may run
        // first, but cannot overtake an earlier unresolved receipt.
        let scopes: Vec<String> =
            sqlx::query_scalar("SELECT scope FROM judge_probe_order WHERE accepted > published")
                .fetch_all(&self.0)
                .await?;
        for scope in scopes {
            let mut tx = self.0.begin().await?;
            let mut cursor: i64 = sqlx::query_scalar(
                "SELECT published FROM judge_probe_order WHERE scope=$1 FOR UPDATE",
            )
            .bind(&scope)
            .fetch_one(&mut *tx)
            .await?;
            loop {
                let next: Option<bool> = sqlx::query_scalar(
                    "SELECT terminal FROM judge_probe_submissions WHERE scope=$1 AND ordinal=$2",
                )
                .bind(&scope)
                .bind(cursor + 1)
                .fetch_optional(&mut *tx)
                .await?;
                if next != Some(true) {
                    break;
                }
                cursor += 1;
                sqlx::query("UPDATE judge_probe_submissions SET published=TRUE WHERE scope=$1 AND ordinal=$2")
                    .bind(&scope).bind(cursor).execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE judge_probe_order SET published=$2 WHERE scope=$1")
                .bind(scope)
                .bind(cursor)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        Ok(())
    }
    pub async fn claim(&self, worker: &str, lease_seconds: f64) -> Result<Option<Lease>> {
        ensure!(
            lease_seconds.is_finite() && lease_seconds > 0.0 && lease_seconds <= 300.0,
            "invalid lease"
        );
        let token = Uuid::new_v4();
        let row = sqlx::query(
            "WITH candidate AS (
            SELECT q.id FROM judge_probe_queue q JOIN workflow_jobs j ON j.id=q.id
            JOIN workflow_pipelines p ON p.id=j.pipeline_id
            JOIN judge_probe_submissions s ON s.id::text=p.id
            WHERE NOT s.terminal AND s.created_at > clock_timestamp()-interval '5 minutes' AND p.state='running' AND j.state IN ('starting','running') AND q.attempt < 3
              AND (q.status='ready' OR (q.status='leased' AND q.deadline <= clock_timestamp()))
            ORDER BY s.created_at,q.id FOR UPDATE OF q SKIP LOCKED LIMIT 1)
            UPDATE judge_probe_queue q SET status='leased',attempt=attempt+1,token=$1,
            worker=$2,deadline=clock_timestamp()+($3 * interval '1 second')
            FROM candidate c WHERE q.id=c.id RETURNING q.id,q.payload,q.attempt",
        )
        .bind(token)
        .bind(worker)
        .bind(lease_seconds)
        .fetch_optional(&self.0)
        .await?;
        row.map(|row| {
            Ok(Lease {
                id: JobId::from(Uuid::parse_str(row.get("id"))?),
                token,
                attempt: row.get("attempt"),
                payload: row.get("payload"),
            })
        })
        .transpose()
    }
    pub async fn complete(&self, lease: &Lease, result: Value) -> Result<bool> {
        ensure!(
            result.to_string().len() <= 12 * 1024 * 1024,
            "result exceeds probe artifact cap"
        );
        // A stale worker cannot overwrite a newer attempt, even before reclaim.
        let n = sqlx::query(
            "UPDATE judge_probe_queue q SET status='done',result=$3,deadline=NULL
            FROM workflow_jobs j JOIN workflow_pipelines p ON p.id=j.pipeline_id
            WHERE q.id=$1 AND q.token=$2 AND q.status='leased' AND q.deadline > clock_timestamp()
                AND j.id=q.id AND p.state='running' AND EXISTS(SELECT 1 FROM judge_probe_submissions s
                    WHERE s.id::text=p.id AND NOT s.terminal AND s.created_at > clock_timestamp()-interval '5 minutes')",
        )
        .bind(lease.id.to_string())
        .bind(lease.token)
        .bind(result)
        .execute(&self.0)
        .await?
        .rows_affected();
        Ok(n == 1)
    }
    pub async fn result(&self, id: JobId) -> Result<Value> {
        let value: Option<Value> = sqlx::query_scalar(
            "SELECT result FROM judge_probe_queue WHERE id=$1 AND status='done'",
        )
        .bind(id.to_string())
        .fetch_optional(&self.0)
        .await?;
        value.context("dependency has no committed result")
    }
    pub async fn status(&self) -> Result<Value> {
        let receipts = sqlx::query("SELECT scope,ordinal,terminal,published,
            extract(epoch FROM (finished_at-created_at))::float8 AS seconds FROM judge_probe_submissions ORDER BY scope,ordinal")
            .fetch_all(&self.0).await?;
        let nodes = sqlx::query(
            "SELECT q.status,q.attempt,q.worker,q.payload->>'phase' AS phase,
            j.state FROM judge_probe_queue q JOIN workflow_jobs j ON j.id=q.id ORDER BY q.id",
        )
        .fetch_all(&self.0)
        .await?;
        Ok(
            json!({"submissions":receipts.iter().map(|r|json!({"scope":r.get::<String,_>("scope"),
            "ordinal":r.get::<i64,_>("ordinal"),"terminal":r.get::<bool,_>("terminal"),
            "published":r.get::<bool,_>("published"),"seconds":r.get::<Option<f64>,_>("seconds")})).collect::<Vec<_>>(),
            "nodes":nodes.iter().map(|r|json!({"status":r.get::<String,_>("status"),"attempt":r.get::<i32,_>("attempt"),
                "worker":r.get::<Option<String>,_>("worker"),"phase":r.get::<Option<String>,_>("phase"),
                "engine_state":r.get::<String,_>("state")})).collect::<Vec<_>>()}),
        )
    }
}

fn validate(definition: &Definition) -> Result<()> {
    let mut incoming: HashMap<JobId, usize> =
        definition.jobs.iter().map(|(id, _)| (*id, 0)).collect();
    ensure!(incoming.len() == definition.jobs.len(), "duplicate job");
    let mut edges = HashSet::new();
    for (from, to) in &definition.edges {
        ensure!(
            incoming.contains_key(from) && incoming.contains_key(to),
            "unknown edge endpoint"
        );
        ensure!(edges.insert((*from, *to)), "duplicate edge");
        *incoming.get_mut(to).expect("checked endpoint") += 1;
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        for (_, to) in definition.edges.iter().filter(|(from, _)| *from == id) {
            let n = incoming.get_mut(to).expect("checked endpoint");
            *n -= 1;
            if *n == 0 {
                ready.push(*to);
            }
        }
    }
    ensure!(visited == definition.jobs.len(), "cyclic graph");
    Ok(())
}

pub fn definition(language: &str, source: &str, input: &str, expected: &str) -> Result<Definition> {
    if !["cpp", "python", "rust"].contains(&language) {
        bail!("unsupported language");
    }
    ensure!(
        source.len() <= 65536 && input.len() <= 65536 && expected.len() <= 65536,
        "fixture too large"
    );
    let compile = JobId::new();
    let run = JobId::new();
    let check = JobId::new();
    Ok(Definition {
        jobs: vec![
            (
                compile,
                json!({"phase":"compile","language":language,"source":source}),
            ),
            (
                run,
                json!({"phase":"run","language":language,"compile":compile,"input":input}),
            ),
            (
                check,
                json!({"phase":"check","run":run,"expected":expected}),
            ),
        ],
        edges: vec![(compile, run), (run, check)],
    })
}
