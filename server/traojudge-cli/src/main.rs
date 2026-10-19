use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {}

fn main() {
    let cli = Cli::parse();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_cli=info".into());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    tracing::debug!("CLI initialized");

    if let Some(command) = cli.command {
        match command {}
    }
}
