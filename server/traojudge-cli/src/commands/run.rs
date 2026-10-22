use std::path::PathBuf;

use anyhow::{Result, ensure};
use traojudge_core::application::judge_contract::one_player::SubmissionParameters;

use crate::{config::GlobalConfig, definition, local};

#[derive(clap::Args)]
pub struct Args {
    #[arg(help = "Problem DAG definition in JSON format")]
    problem: PathBuf,
    #[arg(long, help = "Writer's submission source file")]
    submission: PathBuf,
    #[arg(
        long,
        help = "Submission language ID passed through the core one-player contract"
    )]
    language: Option<String>,
    #[arg(long, help = "New directory for artifacts, logs, and report.json")]
    work_dir: Option<PathBuf>,
}

pub async fn run(_config: &GlobalConfig, args: &Args) -> Result<()> {
    let definition = definition::load(&args.problem)?;
    let submission_environment = SubmissionParameters::new(args.language.clone())?.environment()?;
    let work_dir = args
        .work_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".traojudge/runs").join(uuid::Uuid::new_v4().to_string()));
    let report = local::run(
        &definition,
        &args.problem,
        &args.submission,
        &submission_environment,
        &work_dir,
    )
    .await?;
    println!("Results: {}", report.work_dir.join("report.json").display());
    for node in &report.nodes {
        println!("{}: {:?}", node.name, node.status);
    }
    ensure!(
        report.success(),
        "local DAG execution failed; see report.json and node logs"
    );
    Ok(())
}
