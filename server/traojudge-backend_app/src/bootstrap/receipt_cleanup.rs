use std::{io, num::NonZeroU32, time::Duration};
use tokio::time::{MissedTickBehavior, interval};
use traojudge_core::{
    application::receipt_cleanup::ReceiptCleanupService,
    domain::model::request::ReceiptCleanupBatchSize,
    ports::repository::receipt_cleanup::ReceiptCleanupUnitOfWorkProvider,
};

pub(super) struct Config {
    pub interval: Duration,
    pub batch_size: ReceiptCleanupBatchSize,
}
impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let seconds = positive_env("TRAOJUDGE_RECEIPT_CLEANUP_INTERVAL_SECONDS", "600")?;
        let batch = positive_env("TRAOJUDGE_RECEIPT_CLEANUP_BATCH_SIZE", "1000")?;
        let batch_size = ReceiptCleanupBatchSize::new(batch.get()).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("TRAOJUDGE_RECEIPT_CLEANUP_BATCH_SIZE: {error}"),
            )
        })?;
        Ok(Self {
            interval: Duration::from_secs(u64::from(seconds.get())),
            batch_size,
        })
    }
}
fn positive_env(name: &str, default: &str) -> Result<NonZeroU32, Box<dyn std::error::Error>> {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => default.to_owned(),
        Err(error) => return Err(error.into()),
    };
    value.parse().map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{name}: {error}")).into()
    })
}

pub(super) async fn run<U: ReceiptCleanupUnitOfWorkProvider>(
    service: ReceiptCleanupService<U>,
    period: Duration,
) {
    let mut ticks = interval(period);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        match service.run_batch().await {
            Ok(counts) if counts.editorial_receipts != 0 || counts.contest_receipts != 0 => {
                tracing::info!(
                    editorial_receipts = counts.editorial_receipts,
                    contest_receipts = counts.contest_receipts,
                    "expired request receipts removed"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "request receipt cleanup failed; retrying at the next interval");
            }
        }
    }
}
