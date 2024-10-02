use crate::{error::TicksError, settings::Settings};
use solana_entry::entry::{Entry, EntrySlice};
use solana_runtime::{bank::Bank, transaction_batch::TransactionBatch};
use solana_sdk::transaction::{
    SanitizedTransaction, TransactionError, TransactionVerificationMode,
};
use solana_svm::{
    account_loader::TransactionCheckResult, transaction_error_metrics::TransactionErrorMetrics,
};
use std::{borrow::Cow, sync::Arc};

pub mod error;
pub mod settings;

pub use error::{Error, Result};

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct BankVerifier {
    bank: Arc<Bank>,
    settings: Settings,
}

impl BankVerifier {
    pub fn new(bank: Arc<Bank>, settings: Settings) -> Self {
        Self { bank, settings }
    }

    pub fn get_transactions_sanity_results(
        &self,
        txs: &[SanitizedTransaction],
    ) -> Vec<std::result::Result<(), TransactionError>> {
        txs.iter()
            .map(|tx| {
                let resanitized_tx = self
                    .bank
                    .fully_verify_transaction(tx.to_versioned_transaction())?;
                if resanitized_tx != *tx {
                    // Sanitization before/after epoch give different transaction data - do not execute.
                    return Err(TransactionError::ResanitizationNeeded);
                }
                Ok(())
            })
            .collect::<Vec<_>>()
    }

    pub fn entry_sanity_check(
        &self,
        entry: &Entry,
        verification_mode: TransactionVerificationMode,
    ) -> Result<()> {
        entry
            .transactions
            .iter()
            .map(|tx| self.bank.verify_transaction(tx.clone(), verification_mode))
            .collect::<std::result::Result<Vec<_>, TransactionError>>()?;
        Ok(())
    }

    pub fn batch_and_verify_conflicts<'a, 'b>(
        &'a self,
        sanitized_txs: Cow<'b, [SanitizedTransaction]>,
        transaction_results: impl Iterator<Item = std::result::Result<(), TransactionError>>,
    ) -> TransactionBatch<'a, 'b> {
        let tx_account_lock_limit = self.bank.get_transaction_account_lock_limit();
        let lock_result = if self.settings.switchs.txs_conflict_check {
            self.bank.rc.accounts.lock_accounts_with_results(
                sanitized_txs.iter(),
                transaction_results,
                tx_account_lock_limit,
            )
        } else {
            transaction_results.map(|_| Ok(())).collect()
        };
        let mut result = TransactionBatch::new(lock_result, &self.bank, sanitized_txs);
        if !self.settings.switchs.txs_conflict_check {
            result.set_needs_unlock(false);
        }
        result
    }

    pub fn validate_batch(&self, batch: &TransactionBatch) -> Vec<TransactionCheckResult> {
        let mut error_counters = TransactionErrorMetrics::default();

        let sanitized_txs = batch.sanitized_transactions();
        // check age and cache using the bank directly
        let check_results = self.bank.check_transactions(
            sanitized_txs,
            batch.lock_results(),
            self.settings.max_age,
            &mut error_counters,
        );
        check_results
    }

    pub fn verify_ticks(
        &self,
        entries: &[Entry],
        slot_full: bool,
        tick_hash_count: &mut u64,
    ) -> Result<()> {
        let next_bank_tick_height = self.bank.tick_height() + entries.tick_count();
        let max_bank_tick_height = self.bank.max_tick_height();

        if next_bank_tick_height > max_bank_tick_height {
            warn!("Too many entry ticks found in slot: {}", self.bank.slot());
            return Err(TicksError::TooManyTicks.into());
        }

        if next_bank_tick_height < max_bank_tick_height && slot_full {
            info!("Too few entry ticks found in slot: {}", self.bank.slot());
            return Err(TicksError::TooFewTicks.into());
        }

        if next_bank_tick_height == max_bank_tick_height {
            let has_trailing_entry = entries.last().map(|e| !e.is_tick()).unwrap_or_default();
            if has_trailing_entry {
                warn!("Slot: {} did not end with a tick entry", self.bank.slot());
                return Err(TicksError::TrailingEntry.into());
            }

            if !slot_full {
                warn!("Slot: {} was not marked full", self.bank.slot());
                return Err(TicksError::InvalidLastTick.into());
            }
        }

        let hashes_per_tick = self.bank.hashes_per_tick().unwrap_or(0);
        if !entries.verify_tick_hash_count(tick_hash_count, hashes_per_tick) {
            warn!(
                "Tick with invalid number of hashes found in slot: {}",
                self.bank.slot()
            );
            return Err(TicksError::InvalidTickHashCount.into());
        }

        Ok(())
    }

    pub fn get_batch_results(
        &self,
        transactions: Cow<[SanitizedTransaction]>,
    ) -> Vec<TransactionCheckResult> {
        let transaction_results = if self.settings.switchs.tx_sanity_check {
            self.get_transactions_sanity_results(&transactions)
        } else {
            transactions.iter().map(|_| Ok(())).collect()
        };

        let batch = self.batch_and_verify_conflicts(transactions, transaction_results.into_iter());
        self.validate_batch(&batch)
    }

    pub fn transactions_sanity_check(&self, txs: &[SanitizedTransaction]) -> Result<()> {
        txs.iter()
            .map(|tx| {
                self.bank
                    .fully_verify_transaction(tx.to_versioned_transaction())
            })
            .collect::<std::result::Result<Vec<_>, TransactionError>>()?;
        Ok(())
    }

    pub fn transactions_conflict_check(&self, txs: &[SanitizedTransaction]) -> Result<()> {
        let tx_account_lock_limit = self.bank.get_transaction_account_lock_limit();
        let results = self
            .bank
            .rc
            .accounts
            .lock_accounts(txs.iter(), tx_account_lock_limit);
        results
            .into_iter()
            .collect::<std::result::Result<Vec<_>, TransactionError>>()?;
        Ok(())
    }
}
