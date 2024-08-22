use solana_sdk::{account::ReadableAccount, pubkey::Pubkey};

use crate::{Result, RollupStorage};

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
}
