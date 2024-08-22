use rollups_interface::l2::storage::TransactionsResult;
use solana_sdk::transaction::{SanitizedTransaction, VersionedTransaction};
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;

pub struct TransactionsResultWrapper {
    pub output: LoadAndExecuteSanitizedTransactionsOutput,
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
