#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    traojudge_backend_app::serve().await
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_backend_app=info".into());

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
