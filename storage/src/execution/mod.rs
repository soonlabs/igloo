use crate::{blockstore::txs::CommitBatch, Result, RollupStorage};
use igloo_interface::l2::storage::{TransactionSet, TransactionsResult};
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
        result: TransactionsResultWrapper,
        origin: &CommitBatch,
    ) -> Result<TransactionResults> {
        let executed_txs = result.success_txs(origin.transactions());
        let entries = self.transactions_to_entries(executed_txs)?;

        let bank_result = self.bank_commit(result, &origin, &entries)?;
        self.blockstore_save(entries)?;
        Ok(bank_result)
    }
}

impl TransactionsResult for TransactionsResultWrapper {
    type SuccessIn = SanitizedTransaction;
    type SuccessOut = VersionedTransaction;

    fn success_txs(&self, extras: &[Self::SuccessIn]) -> Vec<Self::SuccessOut> {
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
