use rand::Rng;
use solana_ledger::{
    blockstore::Blockstore,
    shred::{ProcessShredsStats, ReedSolomonCache, Shredder},
};
use solana_sdk::{clock::Slot, hash::Hash, signature::Keypair};
use std::{path::Path, sync::Arc};
use tokio::sync::RwLock;

use super::block::SimpleEntry;

pub type SharedStore = Arc<RwLock<SimpleStore>>;

pub struct SimpleStore {
    inner: Blockstore,
}

impl SimpleStore {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Blockstore::open(path)?,
        })
    }

    pub(crate) fn write_entries(
        &self,
        start_slot: Slot,
        ticks_per_slot: u64,
        parent: Option<u64>,
        is_full_slot: bool,
        keypair: &Keypair,
        entries: Vec<SimpleEntry>,
    ) -> anyhow::Result<usize /*num of data shreds*/> {
        const NUM_TICKS_IN_START_SLOT: u64 = 0;
        const START_INDEX: u32 = 0;
        const VERSION: u16 = 0;

        let mut parent_slot = parent.map_or(start_slot.saturating_sub(1), |v| v);
        let num_slots = (start_slot - parent_slot).max(1); // Note: slot 0 has parent slot 0
        assert!(NUM_TICKS_IN_START_SLOT < num_slots * ticks_per_slot);
        let mut remaining_ticks_in_slot = num_slots * ticks_per_slot - NUM_TICKS_IN_START_SLOT;

        let mut current_slot = start_slot;
        let mut shredder = Shredder::new(current_slot, parent_slot, 0, VERSION).unwrap();
        let mut all_shreds = vec![];
        let mut slot_entries = vec![];
        let reed_solomon_cache = ReedSolomonCache::default();
        let mut chained_merkle_root = Some(Hash::new_from_array(rand::thread_rng().gen()));
        // Find all the entries for start_slot
        for entry in entries.into_iter() {
            if remaining_ticks_in_slot == 0 {
                current_slot += 1;
                parent_slot = current_slot - 1;
                remaining_ticks_in_slot = ticks_per_slot;
                let current_entries = std::mem::take(&mut slot_entries);
                let start_index = {
                    if all_shreds.is_empty() {
                        START_INDEX
                    } else {
                        0
                    }
                };
                let (mut data_shreds, mut coding_shreds) = shredder.entries_to_shreds(
                    keypair,
                    &current_entries,
                    true, // is_last_in_slot
                    chained_merkle_root,
                    start_index, // next_shred_index
                    start_index, // next_code_index
                    true,        // merkle_variant
                    &reed_solomon_cache,
                    &mut ProcessShredsStats::default(),
                );
                all_shreds.append(&mut data_shreds);
                all_shreds.append(&mut coding_shreds);
                chained_merkle_root = Some(coding_shreds.last().unwrap().merkle_root().unwrap());
                shredder = Shredder::new(
                    current_slot,
                    parent_slot,
                    (ticks_per_slot - remaining_ticks_in_slot) as u8,
                    VERSION,
                )
                .unwrap();
            }

            if entry.inner.is_tick() {
                remaining_ticks_in_slot -= 1;
            }
            slot_entries.push(entry.inner);
        }

        if !slot_entries.is_empty() {
            let (mut data_shreds, mut coding_shreds) = shredder.entries_to_shreds(
                keypair,
                &slot_entries,
                is_full_slot,
                chained_merkle_root,
                0,    // next_shred_index
                0,    // next_code_index
                true, // merkle_variant
                &reed_solomon_cache,
                &mut ProcessShredsStats::default(),
            );
            all_shreds.append(&mut data_shreds);
            all_shreds.append(&mut coding_shreds);
        }
        let num_data = all_shreds.iter().filter(|shred| shred.is_data()).count();
        self.inner.insert_shreds(all_shreds, None, false)?;
        Ok(num_data)
    }
}
