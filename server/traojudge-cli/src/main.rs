use clap::{Parser, Subcommand};

mod commands;
mod config;
mod definition;
mod domain;
mod local;
mod ports;
mod ports_impl;

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Apply,
    Validate(commands::validate::Args),
    Run(commands::run::Args),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_cli=info".into());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    tracing::debug!("CLI initialized");

    let global_config = config::GlobalConfig {};

    match cli.command {
        Command::Apply => {
            commands::apply::run(&global_config);
            Ok(())
        }
        Command::Validate(args) => commands::validate::run(&global_config, &args),
        Command::Run(args) => commands::run::run(&global_config, &args).await,
    }
}
