use solana_runtime::installed_scheduler_pool::BankWithScheduler;
use solana_sdk::{account::ReadableAccount, pubkey::Pubkey};

use crate::{error::BankError, Result, RollupStorage};

impl RollupStorage {
    pub fn get_mixed_heights(&self) -> Result<(u64, Option<u64>)> {
        let bank_height = self.bank_forks.read().unwrap().highest_slot();
        let store_height = self.blockstore.highest_slot()?;
        Ok((bank_height, store_height))
    }

    pub fn current_height(&self) -> u64 {
        self.bank.slot()
    }

    pub fn balance(&self, pubkey: &Pubkey) -> u64 {
        self.bank
            .get_account(pubkey)
            .map(|d| d.lamports())
            .unwrap_or(0)
    }

    pub fn reorg(&mut self, slot: u64, finalized: Option<u64>) -> Result<Vec<BankWithScheduler>> {
        let removed = self.set_root(slot, finalized)?;

        // TODO: this should not purge slots that on the best chain
        removed.iter().for_each(|bank| {
            self.blockstore
                .purge_and_compact_slots(bank.slot(), bank.slot());
        });

        let bank_forks = self.bank_forks.read().unwrap();
        self.bank = bank_forks.working_bank();

        Ok(removed)
    }

    pub fn set_root(
        &mut self,
        slot: u64,
        finalized: Option<u64>,
    ) -> Result<Vec<BankWithScheduler>> {
        self.blockstore
            .set_roots(std::iter::once(&slot))
            .map_err(|e| BankError::SetRootFailed(e.to_string()))?;
        let removed_banks = self
            .bank_forks
            .write()
            .unwrap()
            .set_root(
                slot,
                &self.background_service.accounts_background_request_sender,
                finalized,
            )
            .map_err(|e| BankError::SetRootFailed(e.to_string()))?;
        Ok(removed_banks)
    }
}
