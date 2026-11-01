use crate::{
    domain::model::contest::ContestError, ports::repository::contest::ContestUnitOfWorkProvider,
};
use chrono::Duration;
mod management;
mod participation;
mod permissions;
mod problems;
mod roles;

pub struct ContestService<U> {
    unit_of_work: U,
    receipt_retention: Duration,
}
impl<U: ContestUnitOfWorkProvider> ContestService<U> {
    pub fn new(unit_of_work: U, receipt_retention: Duration) -> Result<Self, ContestError> {
        if receipt_retention <= Duration::zero() {
            return Err(ContestError::validation(
                "receipt_retention",
                "must be positive",
            ));
        }
        Ok(Self {
            unit_of_work,
            receipt_retention,
        })
    }
    fn receipt_expires_at(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<chrono::DateTime<chrono::Utc>, ContestError> {
        now.checked_add_signed(self.receipt_retention)
            .ok_or_else(|| ContestError::validation("receipt_retention", "duration is too large"))
    }
}
