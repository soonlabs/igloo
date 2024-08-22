use rollups_interface::l2::{bank::BankInfo, storage::TransactionSet};
use solana_runtime::{
    bank::{Bank, ExecutedTransactionCounts, NewBankOptions, TotalAccountsStats},
    snapshot_bank_utils,
    snapshot_utils::ArchiveFormat,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    clock::Slot,
    pubkey::Pubkey,
    transaction::SanitizedTransaction,
};
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;

use crate::{
    blockstore::txs::CommitBatch,
    error::{AccountDbError, BankError},
    execution::TransactionsResultWrapper,
    Result, RollupStorage,
};

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
            &ledger_path.join("full"),
            &ledger_path.join("incremental"),
            ArchiveFormat::from_cli_arg("zstd")
                .ok_or(BankError::Common("Unsupported archive format".to_string()))?,
        )
        .map_err(|e| BankError::Common(format!("Failed to snapshot bank: {e}").to_string()))?;
        Ok(())
    }

    pub fn bump_slot(&mut self, slot: Slot) {
        let new = Bank::new_from_parent_with_options(
            self.bank.clone(),
            &self.collector_id(),
            slot,
            NewBankOptions {
                vote_only_bank: false,
            },
        );
        let new = self.bank_forks.write().unwrap().insert(new);
        self.bank = new.clone();
    }

    pub(crate) fn bank_commit(
        &mut self,
        mut result: TransactionsResultWrapper,
        batch: &CommitBatch,
    ) -> Result<()> {
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
        self.bank.commit_transactions(
            batch.transactions(),
            &mut result.output.loaded_transactions,
            result.output.execution_results,
            last_blockhash,
            lamports_per_signature,
            counts,
            &mut result.output.execute_timings,
        );

        self.register_ticks();
        Ok(())
    }

    fn register_ticks(&self) {
        let fork = self.bank_forks.read().unwrap();
        let bank_with_schedule = fork.working_bank_with_scheduler();
        // TODO: register real ticks later if use scheduled bank
        for _ in bank_with_schedule.tick_height()..bank_with_schedule.max_tick_height() {
            bank_with_schedule.register_tick(&Default::default());
        }
    }

    fn collect_execution_logs(
        &mut self,
        _sanitized_output: &LoadAndExecuteSanitizedTransactionsOutput,
        _sanitized_txs: &[SanitizedTransaction],
    ) -> ExecutedTransactionCounts {
        // TODO: implement me
        ExecutedTransactionCounts {
            executed_transactions_count: 0,
            executed_non_vote_transactions_count: 0,
            executed_with_failure_result_count: 0,
            signature_count: 0,
        }
    }

    /// Returns true if this account should be included in the output
    fn should_process_account(&self, account: &AccountSharedData) -> bool {
        solana_accounts_db::accounts::Accounts::is_loadable(account.lamports())
            && (!solana_sdk::sysvar::check_id(account.owner()))
    }
}
