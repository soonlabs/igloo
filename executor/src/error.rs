use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    StorageError(#[from] rollups_storage::Error),

    #[error(transparent)]
    ValidatorError(#[from] rollups_validator::Error),
}
