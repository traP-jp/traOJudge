use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;

use crate::config::GlobalConfig;
use crate::definition;

#[derive(clap::Args)]
pub struct Args {
    #[arg(help = "DAG definition in JSON format")]
    path: PathBuf,
    #[arg(long, help = "Print the validated definition as JSON")]
    json: bool,
}

pub fn run(_config: &GlobalConfig, args: &Args) -> Result<()> {
    let definition = definition::load(&args.path)?;
    let mut stdout = io::stdout().lock();
    if args.json {
        serde_json::to_writer_pretty(&mut stdout, &definition)?;
        writeln!(stdout)?;
    } else {
        writeln!(
            stdout,
            "Valid DAG definition ({} DAGs)",
            definition.dags.len()
        )?;
    }
    Ok(())
}
