use rollups_interface::l2::storage::TransactionSet;
use solana_sdk::transaction::SanitizedTransaction;
use std::borrow::Cow;

pub struct CommitBatch<'a> {
    sanitized_txs: Cow<'a, [SanitizedTransaction]>,
}

impl<'a> CommitBatch<'a> {
    pub fn new(sanitized_txs: Cow<'a, [SanitizedTransaction]>) -> Self {
        Self { sanitized_txs }
    }
}

impl<'a> TransactionSet for CommitBatch<'a> {
    type Transaction = SanitizedTransaction;

    fn transactions(&self) -> &[Self::Transaction] {
        &self.sanitized_txs
    }
}
