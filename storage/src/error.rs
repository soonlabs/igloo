use solana_ledger::blockstore::BlockstoreError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Init common error: {0}")]
    InitCommon(String),

    #[error("Load blockstore failed: {0}")]
    LoadBlockstoreFailed(String),

    #[error("Init config failed: {0}")]
    InitConfigFailed(String),

    #[error(transparent)]
    BlockstoreError(#[from] BlockstoreError),
}
