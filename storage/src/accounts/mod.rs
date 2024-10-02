use solana_entry::entry::Entry;
use solana_runtime::{
    bank::{Bank, ExecutedTransactionCounts, NewBankOptions, TotalAccountsStats},
    installed_scheduler_pool::BankWithScheduler,
    snapshot_bank_utils,
    snapshot_utils::ArchiveFormat,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    clock::Slot,
    pubkey::Pubkey,
    transaction::{SanitizedTransaction, VersionedTransaction},
};
use solana_svm::{
    transaction_processor::LoadAndExecuteSanitizedTransactionsOutput,
    transaction_results::TransactionResults,
};
use std::sync::Arc;

use crate::{
    blockstore::txs::CommitBatch,
    error::{AccountDbError, BankError},
    execution::TransactionsResultWrapper,
    BankInfo, Result, RollupStorage,
};

#[cfg(test)]
mod tests;

impl RollupStorage {
    pub fn all_accounts_status(&self) -> Result<TotalAccountsStats> {
        let mut status = TotalAccountsStats::default();
        let rent_collector = self.bank.rent_collector();

        let scan_func = |account_tuple: Option<(&Pubkey, AccountSharedData, Slot)>| {
            if let Some((pubkey, account, _slot)) =
                account_tuple.filter(|(_, account, _)| self.should_process_account(account))
            {
                status.accumulate_account(pubkey, &account, rent_collector);
            }
        };
        self.bank
            .scan_all_accounts(scan_func, true)
            .map_err(|e| AccountDbError::FailedToScanAccounts(e.to_string()))?;

        Ok(status)
    }

    pub fn set_snapshot_interval(&mut self, interval: u64) {
        let mut bank_forks = self.bank_forks.write().unwrap();
        bank_forks.set_accounts_hash_interval_slots(interval);
    }

    pub fn snapshot(&self, slot: Option<Slot>) -> Result<()> {
        let slot = match slot {
            Some(slot) => slot,
            None => self.current_height(),
        };
        let bank = self
            .bank_forks
            .read()
            .unwrap()
            .get(slot)
            .ok_or(BankError::BankNotExists(slot))?;

        let ledger_path = self.config.ledger_path.clone();
        snapshot_bank_utils::bank_to_full_snapshot_archive(
            &ledger_path,
            &bank,
            None,
            ledger_path.join("full"),
            ledger_path.join("incremental"),
            ArchiveFormat::from_cli_arg("zstd")
                .ok_or(BankError::Common("Unsupported archive format".to_string()))?,
        )
        .map_err(|e| BankError::Common(format!("Failed to snapshot bank: {e}").to_string()))?;
        Ok(())
    }

    pub fn current_bank(&self) -> Arc<Bank> {
        self.bank.clone()
    }

    pub fn bump_slot(&mut self, slot: Slot) -> Result<BankWithScheduler> {
        self.new_slot_with_parent(self.bank.clone(), slot)
    }

    pub fn new_slot_with_parent(
        &mut self,
        parent: Arc<Bank>,
        slot: Slot,
    ) -> Result<BankWithScheduler> {
        let new = Bank::new_from_parent_with_options(
            parent,
            &self.collector_id()?,
            slot,
            NewBankOptions {
                vote_only_bank: false,
            },
        );
        let new = self.bank_forks.write().unwrap().insert(new);
        self.bank = new.clone();
        Ok(new)
    }

    pub(crate) fn bank_commit(
        &mut self,
        results: Vec<TransactionsResultWrapper>,
        batches: Vec<CommitBatch>,
        entries: &[Entry],
    ) -> Result<Vec<TransactionResults>> {
        let mut rtn = vec![];
        for (result, batch) in results.into_iter().zip(batches) {
            let result = if self.enable_history() {
                self.single_batch_commit_with_history(result, batch)?
            } else {
                self.single_batch_commit(result, &batch)?
            };

            rtn.push(result);
        }

        self.register_ticks(entries)?;
        Ok(rtn)
    }

    pub(crate) fn single_batch_commit(
        &mut self,
        mut result: TransactionsResultWrapper,
        batch: &CommitBatch,
    ) -> Result<TransactionResults> {
        // In order to avoid a race condition, leaders must get the last
        // blockhash *before* recording transactions because recording
        // transactions will only succeed if the block max tick height hasn't
        // been reached yet. If they get the last blockhash *after* recording
        // transactions, the block max tick height could have already been
        // reached and the blockhash queue could have already been updated with
        // a new blockhash.
        let (last_blockhash, lamports_per_signature) =
            self.bank.last_blockhash_and_lamports_per_signature();

        let counts = self.collect_execution_logs(&result.output, batch.transactions());
        let result = self.bank.commit_transactions(
            batch.transactions(),
            &mut result.output.loaded_transactions,
            result.output.execution_results.clone(),
            last_blockhash,
            lamports_per_signature,
            counts,
            &mut result.output.execute_timings,
        );

        Ok(result)
    }

    pub fn to_sanitized_transaction(
        &self,
        tx: VersionedTransaction,
        verify: bool,
    ) -> Result<SanitizedTransaction> {
        let message_hash = if verify {
            tx.verify_and_hash_message()
                .map_err(|e| AccountDbError::ConvertTxError(e.to_string()))?
        } else {
            tx.message.hash()
        };
        SanitizedTransaction::try_create(
            tx,
            message_hash,
            None,
            self.bank.as_ref(),
            self.bank.get_reserved_account_keys(),
        )
        .map_err(|e| AccountDbError::ConvertTxError(e.to_string()).into())
    }

    fn register_ticks(&self, entries: &[Entry]) -> Result<()> {
        let fork = self.bank_forks.read().unwrap();
        let bank_with_schedule = fork.working_bank_with_scheduler();

        entries.iter().filter(|e| e.is_tick()).for_each(|entry| {
            bank_with_schedule.register_tick(&entry.hash);
        });

        Ok(())
    }

    fn collect_execution_logs(
        &mut self,
        sanitized_output: &LoadAndExecuteSanitizedTransactionsOutput,
        sanitized_txs: &[SanitizedTransaction],
    ) -> ExecutedTransactionCounts {
        let mut signature_count: u64 = 0;
        let mut executed_transactions_count: u64 = 0;
        let mut executed_non_vote_transactions_count: u64 = 0;
        let mut executed_with_failure_result_count: u64 = 0;
        for (execution_result, tx) in sanitized_output
            .execution_results
            .iter()
            .zip(sanitized_txs.as_ref())
        {
            if execution_result.was_executed() {
                // Signature count must be accumulated only if the transaction
                // is executed, otherwise a mismatched count between banking and
                // replay could occur
                signature_count += u64::from(tx.message().header().num_required_signatures);
                executed_transactions_count += 1;

                if !tx.is_simple_vote_transaction() {
                    executed_non_vote_transactions_count += 1;
                }
            }

            if execution_result.flattened_result().is_err() {
                executed_with_failure_result_count += 1;
            }
        }

        ExecutedTransactionCounts {
            executed_transactions_count,
            executed_non_vote_transactions_count,
            executed_with_failure_result_count,
            signature_count,
        }
    }

    /// Returns true if this account should be included in the output
    fn should_process_account(&self, account: &AccountSharedData) -> bool {
        solana_accounts_db::accounts::Accounts::is_loadable(account.lamports())
            && (!solana_sdk::sysvar::check_id(account.owner()))
    }
}
