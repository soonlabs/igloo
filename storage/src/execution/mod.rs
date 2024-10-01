use crate::Error;
use crate::{blockstore::txs::CommitBatch, Result, RollupStorage};
use solana_sdk::transaction::{SanitizedTransaction, VersionedTransaction};
use solana_svm::{
    transaction_processor::LoadAndExecuteSanitizedTransactionsOutput,
    transaction_results::TransactionResults,
};

#[cfg(test)]
mod tests;

pub struct TransactionsResultWrapper {
    pub output: LoadAndExecuteSanitizedTransactionsOutput,
}

impl RollupStorage {
    pub(crate) fn commit_block(
        &mut self,
        result: Vec<TransactionsResultWrapper>,
        origin: Vec<CommitBatch>,
    ) -> Result<Vec<TransactionResults>> {
        if result.len() != origin.len() {
            return Err(Error::CommitBachAndResultsNotMatch);
        }

        // TODO: process entries in parallel in scheduler version

        let mut data_entries = vec![];
        let mut start_hash = None;
        for (result, origin) in result.iter().zip(origin.iter()) {
            let executed_txs = result.success_txs(origin.transactions());
            let entry = self.transactions_to_entry(executed_txs, start_hash)?;
            start_hash = Some(entry.hash);
            data_entries.push(entry);
        }
        let entries = self.complete_entries(data_entries)?;

        let bank_result = self.bank_commit(result, origin, &entries)?;
        self.blockstore_save(entries)?;
        Ok(bank_result)
    }
}

impl TransactionsResultWrapper {
    pub fn success_txs(&self, extras: &[SanitizedTransaction]) -> Vec<VersionedTransaction> {
        self.output
            .execution_results
            .iter()
            .zip(extras)
            .filter_map(|(execution_result, tx)| {
                if execution_result.was_executed() {
                    Some(tx.to_versioned_transaction())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl From<LoadAndExecuteSanitizedTransactionsOutput> for TransactionsResultWrapper {
    fn from(output: LoadAndExecuteSanitizedTransactionsOutput) -> Self {
        Self { output }
    }
}
