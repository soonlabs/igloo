use crate::{
    error::{BankError, StorageError},
    Result, RollupStorage,
};
use solana_runtime::{bank_forks::BankForks, installed_scheduler_pool::BankWithScheduler};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    clock::Slot,
    hash::Hash,
    pubkey::Pubkey,
};
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default)]
pub struct SlotHead {
    pub slot: u64,
    pub hash: Option<Hash>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct SlotInfo {
    pub head: SlotHead,
    pub parent: SlotHead,
    pub store_height: Option<u64>,
}

impl RollupStorage {
    pub fn get_slot_info(&self, slot: u64) -> Result<SlotInfo> {
        let head = self.get_slot_head(slot)?;
        let parent = if head.slot > 0 {
            self.get_slot_head(head.slot - 1)?
        } else {
            Default::default()
        };
        let store_height = self.blockstore.highest_slot()?;
        Ok(SlotInfo {
            head,
            parent,
            store_height,
        })
    }

    pub fn get_slot_head(&self, slot: u64) -> Result<SlotHead> {
        let hash = { self.bank_forks.read().unwrap().get(slot) };
        let timestamp = self.blockstore.get_rooted_block_time(slot).ok();
        hash.map_or_else(
            || {
                Ok(SlotHead {
                    slot,
                    timestamp,
                    ..Default::default()
                })
            },
            |b| {
                Ok(SlotHead {
                    slot,
                    hash: Some(b.hash()),
                    timestamp,
                })
            },
        )
    }

    pub fn get_root(&self) -> u64 {
        self.bank_forks.read().unwrap().root()
    }

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

    pub fn has_account(&self, pubkey: &Pubkey) -> bool {
        self.bank.get_account(pubkey).is_some()
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Result<AccountSharedData> {
        self.bank
            .get_account(pubkey)
            .ok_or(StorageError::AccountNotFound.into())
    }

    pub fn get_account_by_slot(&self, slot: Slot, pubkey: &Pubkey) -> Result<AccountSharedData> {
        self.bank_forks
            .read()
            .unwrap()
            .get(slot)
            .ok_or(BankError::BankNotExists(slot))?
            .get_account(pubkey)
            .ok_or(StorageError::AccountNotFound.into())
    }

    pub fn reorg(&mut self, slot: Slot, finalized: Option<u64>) -> Result<Vec<BankWithScheduler>> {
        let removed = self.set_root(slot, finalized)?;
        let ancestors = find_ancestors(slot, finalized, self.bank_forks.clone(), &removed);

        removed
            .iter()
            .filter(|b| !ancestors.contains(&b.slot()))
            .for_each(|bank| {
                if let Err(e) = self.blockstore.set_dead_slot(bank.slot()) {
                    error!("set dead slot failed: {}", e.to_string());
                }
            });

        let bank_forks = self.bank_forks.read().unwrap();
        self.bank = bank_forks.working_bank();

        Ok(removed)
    }

    pub fn confirm(&mut self, slot: u64) -> Result<()> {
        self.bank.freeze();
        self.blockstore
            .set_roots(std::iter::once(&slot))
            .map_err(|e| StorageError::SetRootFailed(e.to_string()))?;
        Ok(())
    }

    pub fn set_root(
        &mut self,
        slot: u64,
        finalized: Option<u64>,
    ) -> Result<Vec<BankWithScheduler>> {
        self.blockstore
            .set_roots(std::iter::once(&slot))
            .map_err(|e| StorageError::SetRootFailed(e.to_string()))?;
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

pub(crate) fn find_ancestors(
    mut root_slot: u64,
    finalized: Option<u64>,
    bank_forks: Arc<RwLock<BankForks>>,
    removed: &[BankWithScheduler],
) -> HashSet<u64> {
    let forks = bank_forks.read().unwrap();
    let stop_at = finalized.unwrap_or(root_slot);
    while let Some(b) = forks.get(root_slot) {
        if b.parent_slot() >= stop_at {
            root_slot = b.parent_slot();
        } else {
            break;
        }
    }
    let mut root_parent = if let Some(b) = forks.get(root_slot) {
        b.parent_slot()
    } else {
        return Default::default();
    };
    drop(forks);

    let mut ancestors = HashSet::new();
    loop {
        let mut found = false;
        for b in removed {
            let slot = b.slot();
            if !ancestors.contains(&slot) && slot == root_parent {
                ancestors.insert(slot);
                root_parent = b.parent_slot();
                found = true;
                break;
            }
        }

        if !found {
            break;
        }
    }
    ancestors
}
