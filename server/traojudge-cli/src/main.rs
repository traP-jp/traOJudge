use clap::{Parser, Subcommand};

mod commands;
mod config;

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Apply,
    Validate,
}

fn main() {
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
        Command::Apply => commands::apply::run(&global_config),
        Command::Validate => commands::validate::run(&global_config),
    }
}
