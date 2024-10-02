use crate::{
    blockstore::txs::CommitBatch, config::HistoryConfig, execution::TransactionsResultWrapper,
    Result, RollupStorage,
};
use crossbeam_channel::unbounded;
use solana_core::cache_block_meta_service::CacheBlockMetaService;
use solana_ledger::{
    blockstore::Blockstore,
    blockstore_processor::{self, CacheBlockMetaSender, TransactionStatusSender},
};
use solana_rpc::{
    transaction_notifier_interface::TransactionNotifierArc,
    transaction_status_service::TransactionStatusService,
};
use solana_runtime::bank::TransactionBalancesSet;
use solana_sdk::{
    clock::Slot, rent_debits::RentDebits, signature::Signature, transaction::SanitizedTransaction,
};
use solana_svm::transaction_results::{TransactionExecutionResult, TransactionResults};
use solana_transaction_status::{
    token_balances::TransactionTokenBalancesSet, ConfirmedTransactionWithStatusMeta,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc,
};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct StorageHistoryServices {
    pub transaction_status_sender: Option<TransactionStatusSender>,
    pub transaction_status_service: Option<TransactionStatusService>,
    pub cache_block_meta_sender: Option<CacheBlockMetaSender>,
    pub cache_block_meta_service: Option<CacheBlockMetaService>,
    pub max_complete_transaction_status_slot: Arc<AtomicU64>,
}

pub struct TransactionBatchHistoryInfo {
    pub transactions: Vec<SanitizedTransaction>,
    pub execution_results: Vec<TransactionExecutionResult>,
    pub balances: TransactionBalancesSet,
    pub token_balances: TransactionTokenBalancesSet,
    pub rent_debits: Vec<RentDebits>,
    pub transaction_indexes: Vec<usize>,
}

impl RollupStorage {
    pub fn enable_history(&self) -> bool {
        self.history_services.transaction_status_sender.is_some()
            && self.history_services.cache_block_meta_sender.is_some()
    }

    pub fn get_transaction_meta(
        &self,
        signature: Signature,
        highest_confirmed_slot: Option<Slot>,
    ) -> Result<Option<ConfirmedTransactionWithStatusMeta>> {
        if let Some(highest_comfirmed_slot) = highest_confirmed_slot {
            self.blockstore
                .get_complete_transaction(signature, highest_comfirmed_slot)
        } else {
            self.blockstore.get_rooted_transaction(signature)
        }
        .map_err(|e| e.into())
    }

    pub fn single_batch_commit_with_history(
        &mut self,
        result: TransactionsResultWrapper,
        mut origin: CommitBatch,
    ) -> Result<TransactionResults> {
        let execution_results = result.output.execution_results.clone();

        let pre_balances = origin.collect_balances(self.bank.clone());
        let pre_token_balances = origin.collect_token_balances(self.bank.clone());

        let batch_result = self.single_batch_commit(result, &origin)?;

        let post_balances = origin.collect_balances(self.bank.clone());
        let post_token_balances = origin.collect_token_balances(self.bank.clone());
        let history_info = TransactionBatchHistoryInfo {
            transactions: origin.sanitized_txs.to_vec(),
            execution_results,
            balances: TransactionBalancesSet::new(pre_balances, post_balances),
            token_balances: TransactionTokenBalancesSet::new(
                pre_token_balances,
                post_token_balances,
            ),
            transaction_indexes: origin.transaction_indexes,
            rent_debits: batch_result.rent_debits.clone(),
        };
        self.on_block_complete(history_info);

        Ok(batch_result)
    }

    pub fn notify_block_complete(&self) {
        blockstore_processor::cache_block_meta(
            &self.bank,
            self.history_services.cache_block_meta_sender.as_ref(),
        );
    }

    pub fn send_transaction_history_status(&self, history_info: TransactionBatchHistoryInfo) {
        if let Some(sender) = self.history_services.transaction_status_sender.as_ref() {
            sender.send_transaction_status_batch(
                self.bank.clone(),
                history_info.transactions,
                history_info.execution_results,
                history_info.balances,
                history_info.token_balances,
                history_info.rent_debits,
                history_info.transaction_indexes,
            )
        }
    }
}

impl StorageHistoryServices {
    pub fn new(
        blockstore: Arc<Blockstore>,
        exit: Arc<AtomicBool>,
        config: &HistoryConfig,
        transaction_notifier: Option<TransactionNotifierArc>,
    ) -> Self {
        let max_complete_transaction_status_slot = Arc::new(AtomicU64::new(blockstore.max_root()));
        let (transaction_status_sender, transaction_status_receiver) = unbounded();
        let transaction_status_sender = Some(TransactionStatusSender {
            sender: transaction_status_sender,
        });
        let transaction_status_service = Some(TransactionStatusService::new(
            transaction_status_receiver,
            max_complete_transaction_status_slot.clone(),
            config.enable_transaction_history,
            transaction_notifier,
            blockstore.clone(),
            config.enable_extended_tx_metadata_storage,
            exit.clone(),
        ));

        let (cache_block_meta_sender, cache_block_meta_receiver) = unbounded();
        let cache_block_meta_sender = Some(cache_block_meta_sender);
        let cache_block_meta_service = Some(CacheBlockMetaService::new(
            cache_block_meta_receiver,
            blockstore,
            exit,
        ));
        Self {
            transaction_status_sender,
            transaction_status_service,
            cache_block_meta_sender,
            cache_block_meta_service,
            max_complete_transaction_status_slot,
        }
    }
}
