#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedRequest<T>(pub T);

pub trait Validator {
    fn validate(&self) -> anyhow::Result<()>;
}
