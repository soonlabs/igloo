use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Storage is none")]
    StorageIsNone,

    #[error("Fetch stream batch error: {0}")]
    FetchStreamBatchError(String),

    #[error(transparent)]
    StorageError(#[from] igloo_storage::Error),

    #[error(transparent)]
    ValidatorError(#[from] igloo_verifier::Error),

    #[error("Storage query error: {0}")]
    StorageQueryError(String),
}
