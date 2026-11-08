//! Internal AWS judge control plane. Public Web/CLI authentication remains in backend_app.
mod protocol;
use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use protocol::{Definition, NodeResult, ObjectRef, Task, Template};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use traojudge_core::{
    application::judge_contract::one_player::OnePlayerJudgeContract,
    domain::model::judge_dag::ExecutionNode, ports::judge_payload_builder::JudgePayloadBuilder,
};
use traojudge_infra::postgres::{self, workflow::PostgresUnitOfWorkProvider};
use traojudge_workflow::{
    Engine, JobId, JobStartCallback, PipelineId,
    payload::{Payload, PayloadError, Revision0},
};
use uuid::Uuid;

#[derive(Clone)]
struct Dispatch {
    pool: PgPool,
    sqs: aws_sdk_sqs::Client,
    queue: String,
}
#[async_trait]
impl JobStartCallback for Dispatch {
    type Input = Revision0<Template>;
    type Error = std::io::Error;
    async fn start(&self, id: JobId, payload: Payload<Self::Input>) -> std::io::Result<()> {
        self.send(id, payload.into_value())
            .await
            .map_err(std::io::Error::other)
    }
}
impl Dispatch {
    async fn send(&self, id: JobId, t: Template) -> Result<()> {
        let mut inputs = BTreeMap::new();
        for (name, reference) in t.inputs {
            let reference = match reference {
                Some(r) => r,
                None => {
                    let value: Value = sqlx::query_scalar(
                        "SELECT reference FROM judge_artifacts WHERE run_id=$1 AND name=$2",
                    )
                    .bind(&t.run_id)
                    .bind(&name)
                    .fetch_one(&self.pool)
                    .await?;
                    serde_json::from_value(value)?
                }
            };
            inputs.insert(name, reference);
        }
        let task = Task {
            schema_version: 1,
            run_id: t.run_id,
            job_id: id.to_string(),
            image: t.image,
            execution: t.execution,
            inputs,
        };
        let body = serde_json::to_string(&task)?;
        ensure!(body.len() <= 128 * 1024, "task exceeds queue contract");
        self.sqs
            .send_message()
            .queue_url(&self.queue)
            .message_body(body)
            .send()
            .await?;
        sqlx::query("INSERT INTO judge_events(run_id,job_id,node_name,kind) VALUES($1,$2,$3,'dispatched') ON CONFLICT(job_id,kind) DO UPDATE SET observations=judge_events.observations+1")
            .bind(&task.run_id).bind(id.to_string()).bind(&task.execution.name).execute(&self.pool).await?;
        tracing::info!(job_id=%id, node=%task.execution.name, "node dispatched");
        Ok(())
    }
}
struct Builder {
    run_id: String,
    definition: Definition,
}
impl JudgePayloadBuilder for Builder {
    type Input = Revision0<Template>;
    fn make_payload(
        &mut self,
        index: usize,
        _execution: &ExecutionNode,
    ) -> Result<Payload<Self::Input>, PayloadError> {
        let execution = self.definition.executions[index].clone();
        let inputs = execution
            .inputs
            .iter()
            .map(|i| {
                (
                    i.artifact.clone(),
                    self.definition
                        .artifacts
                        .iter()
                        .find(|a| a.name == i.artifact)
                        .unwrap()
                        .reference
                        .clone(),
                )
            })
            .collect();
        Payload::new(Template {
            run_id: self.run_id.clone(),
            image: self.definition.image.clone(),
            execution,
            inputs,
        })
    }
}
struct Service {
    pool: PgPool,
    engine: Engine<PostgresUnitOfWorkProvider, Dispatch>,
    s3: aws_sdk_s3::Client,
    sqs: aws_sdk_sqs::Client,
    bucket: String,
    image: String,
    results_queue: String,
}
fn job_ids(run: Uuid, d: &Definition) -> Vec<JobId> {
    d.executions
        .iter()
        .map(|e| {
            let mut h = Sha256::new();
            h.update(run.as_bytes());
            h.update(e.name.as_bytes());
            let digest = h.finalize();
            let mut bytes = [0; 16];
            bytes.copy_from_slice(&digest[..16]);
            // Deterministic UUIDs are scoped to a run; explicit rejudging uses a new run UUID.
            bytes[6] = (bytes[6] & 15) | 0x80;
            bytes[8] = (bytes[8] & 63) | 0x80;
            JobId::from(Uuid::from_bytes(bytes))
        })
        .collect()
}
impl Service {
    async fn start(&self, run: Uuid, definition: Definition) -> Result<()> {
        let ids = job_ids(run, &definition);
        let dag = definition.domain()?;
        OnePlayerJudgeContract
            .start_dag_with_ids(
                &self.engine,
                &dag,
                &mut Builder {
                    run_id: run.to_string(),
                    definition,
                },
                PipelineId::from(run),
                ids,
            )
            .await?;
        Ok(())
    }
    async fn verify_object(&self, r: &ObjectRef) -> Result<()> {
        r.validate(&self.bucket)?;
        let object = self
            .s3
            .head_object()
            .bucket(&r.bucket)
            .key(&r.key)
            .version_id(&r.version_id)
            .checksum_mode(aws_sdk_s3::types::ChecksumMode::Enabled)
            .send()
            .await?;
        ensure!(
            object.content_length() == Some(i64::try_from(r.byte_length)?),
            "object length mismatch"
        );
        let raw = (0..64)
            .step_by(2)
            .map(|i| u8::from_str_radix(&r.sha256[i..i + 2], 16))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            object.checksum_sha256() == Some(STANDARD.encode(raw).as_str()),
            "object SHA-256 mismatch"
        );
        Ok(())
    }
    async fn accept(&self, r: NodeResult) -> Result<()> {
        ensure!(
            r.schema_version == 1 && ["completed", "failed"].contains(&r.outcome.as_str()),
            "invalid result envelope"
        );
        let id = JobId::from(Uuid::parse_str(&r.job_id)?);
        let stored: Value = sqlx::query_scalar(
            "SELECT payload::jsonb FROM workflow_jobs WHERE id=$1 AND pipeline_id=$2",
        )
        .bind(&r.job_id)
        .bind(&r.run_id)
        .fetch_one(&self.pool)
        .await?;
        let t: Payload<Revision0<Template>> = serde_json::from_value(stored)?;
        ensure!(
            t.value().run_id == r.run_id && t.value().execution.name == r.node,
            "result identity mismatch"
        );
        if r.outcome == "completed" {
            ensure!(
                r.result.as_ref().is_some_and(Value::is_object),
                "result must be a JSON object"
            );
            let mut actual: Vec<_> = r.outputs.keys().cloned().collect();
            let mut expected = t.value().execution.outputs.clone();
            actual.sort();
            expected.sort();
            ensure!(actual == expected, "declared output set mismatch");
        } else {
            ensure!(
                r.outputs.is_empty(),
                "failed results must not publish outputs"
            );
        }
        for reference in r.outputs.values().chain(r.logs.values()) {
            ensure!(
                reference
                    .key
                    .starts_with(&format!("runs/{}/{}/", r.run_id, r.job_id)),
                "foreign result object"
            );
            self.verify_object(reference).await?;
        }
        let mut tx = self.pool.begin().await?;
        let inserted=sqlx::query("INSERT INTO judge_results(job_id,run_id,node_name,body) VALUES($1,$2,$3,$4) ON CONFLICT(job_id) DO NOTHING")
            .bind(&r.job_id).bind(&r.run_id).bind(&r.node).bind(serde_json::to_value(&r)?).execute(&mut *tx).await?;
        if inserted.rows_affected() == 1 {
            sqlx::query("INSERT INTO judge_events(run_id,job_id,node_name,kind) VALUES($1,$2,$3,'result_received')")
                .bind(&r.run_id).bind(&r.job_id).bind(&r.node).execute(&mut *tx).await?;
            for (name, reference) in &r.outputs {
                sqlx::query("INSERT INTO judge_artifacts(run_id,name,reference) VALUES($1,$2,$3)")
                    .bind(&r.run_id)
                    .bind(name)
                    .bind(serde_json::to_value(reference)?)
                    .execute(&mut *tx)
                    .await?;
            }
        } else {
            let existing: Value =
                sqlx::query_scalar("SELECT body FROM judge_results WHERE job_id=$1")
                    .bind(&r.job_id)
                    .fetch_one(&mut *tx)
                    .await?;
            ensure!(
                existing == serde_json::to_value(&r)?,
                "conflicting duplicate result"
            );
        }
        tx.commit().await?;
        // Durable inbox and artifact references precede dependency release. Replay repairs a
        // crash between these transactions; Engine's CAS makes duplicate application harmless.
        self.apply(id, &r.outcome).await
    }
    async fn apply(&self, id: JobId, outcome: &str) -> Result<()> {
        if outcome == "completed" {
            self.engine.job_completed(id).await?;
        } else {
            self.engine.job_failed(id).await?;
        }
        sqlx::query("UPDATE judge_results SET applied=true WHERE job_id=$1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    async fn reconcile(&self) -> Result<()> {
        let pending: Vec<(String, Value)> = sqlx::query_as(
            "SELECT job_id,body FROM judge_results WHERE NOT applied ORDER BY received_at",
        )
        .fetch_all(&self.pool)
        .await?;
        for (id, v) in pending {
            let r: NodeResult = serde_json::from_value(v)?;
            self.apply(JobId::from(Uuid::parse_str(&id)?), &r.outcome)
                .await?;
        }
        let active:Vec<(String,Value)>=sqlx::query_as("SELECT r.id,r.definition FROM judge_runs r LEFT JOIN workflow_pipelines p ON p.id=r.id WHERE p.id IS NULL OR p.state='running' ORDER BY r.created_at")
            .fetch_all(&self.pool).await?;
        for (id, v) in active {
            self.start(Uuid::parse_str(&id)?, serde_json::from_value(v)?)
                .await?;
        }
        Ok(())
    }
    async fn consume(&self) -> Result<()> {
        let messages = self
            .sqs
            .receive_message()
            .queue_url(&self.results_queue)
            .wait_time_seconds(10)
            .max_number_of_messages(10)
            .send()
            .await?;
        for m in messages.messages() {
            let accepted = match m.body() {
                Some(body) => match serde_json::from_str(body) {
                    Ok(r) => self.accept(r).await,
                    Err(e) => Err(e.into()),
                },
                None => Err(anyhow::anyhow!("empty result")),
            };
            match accepted {
                Ok(()) => {
                    self.sqs
                        .delete_message()
                        .queue_url(&self.results_queue)
                        .receipt_handle(m.receipt_handle().context("missing receipt")?)
                        .send()
                        .await?;
                }
                Err(error) => {
                    tracing::error!(%error,"result not acknowledged; retry/DLQ will retain it")
                }
            }
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submit {
    run_id: Uuid,
    definition: Definition,
}
type ApiError = (StatusCode, Json<Value>);
fn api_error(e: anyhow::Error) -> ApiError {
    tracing::error!(error=%e,"request failed");
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":e.to_string()})),
    )
}
async fn submit(
    State(s): State<Arc<Service>>,
    Json(request): Json<Submit>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let result = async {
        request.definition.validate(&s.bucket, &s.image)?;
        for a in &request.definition.artifacts {
            if let Some(r) = &a.reference {
                s.verify_object(r).await?;
            }
        }
        let value = serde_json::to_value(&request.definition)?;
        sqlx::query(
            "INSERT INTO judge_runs(id,definition) VALUES($1,$2) ON CONFLICT(id) DO NOTHING",
        )
        .bind(request.run_id.to_string())
        .bind(&value)
        .execute(&s.pool)
        .await?;
        let existing: Value = sqlx::query_scalar("SELECT definition FROM judge_runs WHERE id=$1")
            .bind(request.run_id.to_string())
            .fetch_one(&s.pool)
            .await?;
        ensure!(existing == value, "run ID already has another definition");
        // Registration is durable before dispatch. A temporary SQS error is retried by reconciliation.
        if let Err(error) = s.start(request.run_id, request.definition).await {
            tracing::error!(%error,"durable run awaits reconciliation");
        }
        Ok::<_, anyhow::Error>((StatusCode::ACCEPTED, Json(json!({"run_id":request.run_id}))))
    }
    .await;
    result.map_err(api_error)
}
async fn status(
    State(s): State<Arc<Service>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let result=async {
        let definition:Option<Value>=sqlx::query_scalar("SELECT definition FROM judge_runs WHERE id=$1").bind(id.to_string()).fetch_optional(&s.pool).await?;
        let Some(definition)=definition else {return Ok(Json(json!({"error":"run not found"})));};
        let d:Definition=serde_json::from_value(definition)?;
        let state:Option<String>=sqlx::query_scalar("SELECT state FROM workflow_pipelines WHERE id=$1").bind(id.to_string()).fetch_optional(&s.pool).await?;
        let jobs:Vec<(String,String,Value)>=sqlx::query_as("SELECT id,state,payload::jsonb FROM workflow_jobs WHERE pipeline_id=$1 ORDER BY id").bind(id.to_string()).fetch_all(&s.pool).await?;
        let results:Vec<(String,Value)>=sqlx::query_as("SELECT job_id,body FROM judge_results WHERE run_id=$1").bind(id.to_string()).fetch_all(&s.pool).await?;
        let results:BTreeMap<_,_>=results.into_iter().collect();
        let events:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('node',node_name,'kind',kind,'observations',observations,'at',created_at) FROM judge_events WHERE run_id=$1 ORDER BY id")
            .bind(id.to_string()).fetch_all(&s.pool).await?;
        let mut cases=0;
        let nodes:Vec<_>=jobs.into_iter().map(|(job_id,state,payload)| {
            let name=payload["json"]["execution"]["name"].as_str().unwrap_or("");
            if state=="completed" && d.executions.iter().any(|e|e.name==name&&e.case_id.is_some()){cases+=1;}
            json!({"job_id":job_id,"name":name,"state":state,"result":results.get(&job_id)})
        }).collect();
        Ok::<_,anyhow::Error>(Json(json!({"run_id":id,"state":state.unwrap_or("registering".into()),"completed_case_count":cases,"total_case_count":d.executions.iter().filter(|e|e.case_id.is_some()).count(),"nodes":nodes,"events":events})))
    }.await;
    result.map_err(api_error)
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("traojudge_judge=info")
        .init();
    let config: aws_types::SdkConfig =
        aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3 = aws_sdk_s3::Client::new(&config);
    let sqs = aws_sdk_sqs::Client::new(&config);
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            let secret = aws_sdk_secretsmanager::Client::new(&config)
                .get_secret_value()
                .secret_id(std::env::var("DATABASE_SECRET_ARN")?)
                .send()
                .await?;
            let v: Value = serde_json::from_str(
                secret
                    .secret_string()
                    .context("database secret is not JSON")?,
            )?;
            format!(
                "postgres://{}:{}@{}:5432/judge?sslmode=verify-full&sslrootcert=/etc/ssl/certs/aws-rds.pem",
                v["username"].as_str().context("username")?,
                v["password"].as_str().context("password")?,
                std::env::var("DATABASE_HOST")?
            )
        }
    };
    let pool = postgres::connect(&database_url).await?;
    postgres::MIGRATOR.run(&pool).await?;
    let dispatch = Dispatch {
        pool: pool.clone(),
        sqs: sqs.clone(),
        queue: std::env::var("REQUEST_QUEUE_URL")?,
    };
    let service = Arc::new(Service {
        pool: pool.clone(),
        engine: Engine::new(PostgresUnitOfWorkProvider::new(pool), dispatch),
        s3,
        sqs,
        bucket: std::env::var("ARTIFACT_BUCKET")?,
        image: std::env::var("RUNTIME_IMAGE")?,
        results_queue: std::env::var("RESULT_QUEUE_URL")?,
    });
    let background = service.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = background.reconcile().await {
                tracing::error!(%error,"reconciliation failed");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            if let Err(error) = background.consume().await {
                tracing::error!(%error,"result receive failed");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    });
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/runs", post(submit))
        .route("/runs/{id}", get(status))
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024))
        .with_state(service);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
