#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    traojudge_backend_app::bootstrap::init_tracing();
    traojudge_backend_app::bootstrap::serve().await
}
