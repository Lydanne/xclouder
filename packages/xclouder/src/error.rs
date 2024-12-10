use thiserror::Error;

pub type XResult<T> = std::result::Result<T, XError>;

#[derive(Error, Debug, Clone)]
pub enum XError {
    #[error("Cloud not found")]
    CloudNotFound,
    
    #[error("Bucket {0} not found")]
    BucketNotFound(String),
    
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Invalid config")]
    InvalidConfig,
    
    #[error("Serde error: {0}")]
    SerdeError(String),
    
    #[error("Lock error: {0}")]
    LockError(String),
}

impl From<serde_json::Error> for XError {
    fn from(err: serde_json::Error) -> Self {
        XError::SerdeError(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for XError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        XError::LockError(err.to_string())
    }
} 