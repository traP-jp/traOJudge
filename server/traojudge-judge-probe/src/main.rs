use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, time::Duration};
use traojudge_judge_probe::{
    queue::{Queue, definition},
    runtime::Runtime,
};
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    Init,
    Submit {
        id: Uuid,
        scope: String,
        fixture: PathBuf,
    },
    Tick,
    Controller,
    Status,
    Events {
        scope: String,
        #[arg(long, default_value_t = 0)]
        after: i64,
    },
    Worker {
        #[arg(long)]
        once: bool,
    },
}
#[derive(Deserialize)]
struct Fixture {
    language: String,
    source: String,
    input: String,
    expected: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let q = Queue(
        PgPoolOptions::new()
            .max_connections(12)
            .connect(&std::env::var("DATABASE_URL").context("DATABASE_URL required")?)
            .await?,
    );
    match args.command {
        Action::Init => q.init().await?,
        Action::Submit { id, scope, fixture } => {
            let fixture: Fixture = serde_json::from_slice(&tokio::fs::read(fixture).await?)?;
            let mut graph = definition(
                &fixture.language,
                &fixture.source,
                &fixture.input,
                &fixture.expected,
            )?;
            use sha2::{Digest, Sha256};
            let replacements = graph
                .jobs
                .iter()
                .enumerate()
                .map(|(i, (old, _))| {
                    let mut hash = Sha256::new();
                    hash.update(id.as_bytes());
                    hash.update((i as u64).to_be_bytes());
                    let mut bytes = [0u8; 16];
                    bytes.copy_from_slice(&hash.finalize()[..16]);
                    (
                        *old,
                        traojudge_workflow::JobId::from(Uuid::from_bytes(bytes)),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>();
            for (job, _) in &mut graph.jobs {
                *job = replacements[job];
            }
            for (from, to) in &mut graph.edges {
                *from = replacements[from];
                *to = replacements[to];
            }
            // Dependency references are generated from the same deterministic IDs.
            graph.jobs[1].1["compile"] = serde_json::to_value(graph.jobs[0].0)?;
            graph.jobs[2].1["run"] = serde_json::to_value(graph.jobs[1].0)?;
            println!(
                "{}",
                json!({"id":id,"ordinal":q.accept(id,&scope,&graph).await?})
            );
        }
        Action::Tick => q.reconcile().await?,
        Action::Controller => {
            let mut leader = q.0.acquire().await?;
            loop {
                let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(44002)")
                    .fetch_one(&mut *leader)
                    .await?;
                if locked {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            loop {
                // A broken leader connection must stop this controller, not let
                // it run concurrently after its session lock has been lost.
                sqlx::query("SELECT 1").execute(&mut *leader).await?;
                q.reconcile().await?;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        Action::Status => println!("{}", q.status().await?),
        Action::Events { scope, after } => println!("{}", q.events(&scope, after, 256).await?),
        Action::Worker { once } => {
            let runtime = Runtime {
                flake: std::env::var("JUDGE_FLAKE")
                    .unwrap_or("/opt/judge-command".into())
                    .into(),
                work: std::env::var("JUDGE_WORK")
                    .unwrap_or("/var/lib/judge-probe/jobs".into())
                    .into(),
                cpu: std::env::var("JUDGE_CPU").unwrap_or("1".into()),
            };
            let worker = std::env::var("JUDGE_WORKER").context("JUDGE_WORKER required")?;
            loop {
                if let Some(lease) = q.claim(&worker, 30.0).await? {
                    match runtime.execute(&q, &lease).await {
                        Ok(result) => {
                            if !q.complete(&lease, result).await? {
                                eprintln!("lease expired before commit");
                            }
                        }
                        Err(error) => {
                            if error.to_string().starts_with("FATAL_CLEANUP") {
                                return Err(error);
                            }
                            eprintln!("attempt {} failed: {error:#}", lease.attempt);
                        }
                    }
                } else if !once {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                if once {
                    break;
                }
            }
        }
    }
    Ok(())
}
