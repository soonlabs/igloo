use rollups_interface::l2::bank::BankInfo;
use solana_runtime::bank::{Bank, NewBankOptions};
use solana_sdk::clock::Slot;

use crate::RollupStorage;

impl RollupStorage {
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
}
