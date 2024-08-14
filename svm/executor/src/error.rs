use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Builder Error: {0}")]
    BuilderError(String),

    #[error("Transaction processor is none")]
    TransactionProcessorIsNone,

    #[error(transparent)]
    StdIo(#[from] std::io::Error),

    #[error(transparent)]
    SolanaTransactionError(#[from] solana_sdk::transaction::TransactionError),
}
