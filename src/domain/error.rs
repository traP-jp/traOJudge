use std::error;
use std::fmt;

#[derive(Debug)]
pub struct AppError {
    pub code: u16,
    pub message: String,
    pub error: Option<Box<dyn error::Error + Send + Sync>>,
}

impl AppError {
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        AppError {
            code,
            message: message.into(),
            error: None,
        }
    }

    pub fn with_source<E>(code: u16, message: impl Into<String>, error: E) -> Self
    where
        E: error::Error + Send + Sync + 'static,
    {
        AppError {
            code,
            message: message.into(),
            error: Some(Box::new(error)),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Error {}]: {}", self.code, self.message)
    }
}