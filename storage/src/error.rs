use solana_ledger::blockstore::BlockstoreError;
use solana_sdk::clock::Slot;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Init common error: {0}")]
    InitCommon(String),

    #[error("Load blockstore failed: {0}")]
    LoadBlockstoreFailed(String),

    #[error("Init bank forks failed: {0}")]
    InitBankForksFailed(String),

    #[error("Init config failed: {0}")]
    InitConfigFailed(String),

    #[error("Keypairs config missing validator keypair")]
    KeypairsConfigMissingValidatorKeypair,

    #[error("Commit batch and results not match")]
    CommitBachAndResultsNotMatch,

    #[error("No entries")]
    NoEntries,

    #[error(transparent)]
    SolanaBlockstoreError(#[from] BlockstoreError),

    #[error(transparent)]
    StorageError(#[from] StorageError),

    #[error(transparent)]
    BankError(#[from] BankError),

    #[error(transparent)]
    AccountDbError(#[from] AccountDbError),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Too many shreds")]
    TooManyShreds,

    #[error("Shred not found, slot: {slot}, index: {index}")]
    ShredNotFound { slot: Slot, index: u64 },

    #[error("Unknown slot meta, slot: {0}")]
    UnknownSlotMeta(Slot),

    #[error("Unknown last index, slot: {0}")]
    UnknownLastIndex(Slot),

    #[error("Invalid Merkle root, slot: {slot}, index: {index}")]
    InvalidMerkleRoot { slot: Slot, index: u64 },

    #[error("Empty entries hashes")]
    EmptyEntriesHashes,

    #[error("Blockstore set root failed: {0}")]
    SetRootFailed(String),

    #[error("Account not found")]
    AccountNotFound,
}

#[derive(Debug, Error)]
pub enum BankError {
    #[error("Bank common error: {0}")]
    Common(String),

    #[error("Bank set root failed: {0}")]
    SetRootFailed(String),

    #[error("Bank at slot {0} not found")]
    BankNotExists(Slot),

    #[error("Invalid operation in bank: {0}")]
    InvalidOperation(String),
}

#[derive(Debug, Error)]
pub enum AccountDbError {
    #[error("Failed to scan accounts: {0}")]
    FailedToScanAccounts(String),

    #[error("Convert transaction error: {0}")]
    ConvertTxError(String),
}
