use crate::{error::StorageError, Error, Result};
use solana_entry::entry::Entry;
use solana_ledger::{
    blockstore::{Blockstore, MAX_DATA_SHREDS_PER_SLOT},
    shred::{
        self, shred_code::MAX_CODE_SHREDS_PER_SLOT, ProcessShredsStats, ReedSolomonCache, Shred,
        Shredder,
    },
};
use solana_measure::measure::Measure;
use solana_runtime::bank::Bank;
use solana_sdk::{clock::Slot, hash::Hash, signature::Keypair};
use std::sync::Arc;

#[derive(Clone)]
pub struct EntriesProcessor {
    slot: Slot,
    parent: Slot,
    chained_merkle_root: Hash,
    next_shred_index: u32,
    next_code_index: u32,
    // If last_tick_height has reached bank.max_tick_height() for this slot
    // and so the slot is completed and all shreds are already broadcast.
    completed: bool,
    process_shreds_stats: ProcessShredsStats,
    shred_version: u16,
    num_batches: usize,
    reed_solomon_cache: Arc<ReedSolomonCache>,
}

impl EntriesProcessor {
    pub(super) fn new(shred_version: u16) -> Self {
        Self {
            slot: Slot::MAX,
            parent: Slot::MAX,
            chained_merkle_root: Hash::default(),
            next_shred_index: 0,
            next_code_index: 0,
            completed: true,
            process_shreds_stats: ProcessShredsStats::default(),
            shred_version,
            num_batches: 0,
            reed_solomon_cache: Arc::<ReedSolomonCache>::default(),
        }
    }

    pub fn process(
        &mut self,
        entries: &[Entry],
        bank: Arc<Bank>,
        blockstore: &Blockstore,
        last_tick_height: u64,
        keypair: &Keypair,
    ) -> Result<(
        Vec<Shred>, // data shreds
        Vec<Shred>, // coding shreds
    )> {
        let mut process_stats = ProcessShredsStats::default();
        let mut to_shreds_time = Measure::start("broadcast_to_shreds");

        if self.slot != bank.slot() {
            self.reset(bank.clone(), blockstore, &mut process_stats);
        }

        let is_last_in_slot = last_tick_height == bank.max_tick_height();
        let reference_tick = bank.tick_height() % bank.ticks_per_slot();

        let (data_shreds, coding_shreds) = self.entries_to_shreds(
            keypair,
            entries,
            reference_tick as u8,
            is_last_in_slot,
            &mut process_stats,
            MAX_DATA_SHREDS_PER_SLOT as u32,
            MAX_CODE_SHREDS_PER_SLOT as u32,
        )?;
        to_shreds_time.stop();

        // Increment by two batches, one for the data batch, one for the coding batch.
        self.num_batches += 2;

        process_stats.shredding_elapsed = to_shreds_time.as_us();
        self.process_shreds_stats += process_stats;

        if last_tick_height == bank.max_tick_height() {
            self.completed = true;
        }

        Ok((data_shreds, coding_shreds))
    }

    #[allow(clippy::too_many_arguments)]
    fn entries_to_shreds(
        &mut self,
        keypair: &Keypair,
        entries: &[Entry],
        reference_tick: u8,
        is_slot_end: bool,
        process_stats: &mut ProcessShredsStats,
        max_data_shreds_per_slot: u32,
        max_code_shreds_per_slot: u32,
    ) -> Result<(
        Vec<Shred>, // data shreds
        Vec<Shred>, // coding shreds
    )> {
        let (data_shreds, coding_shreds) =
            Shredder::new(self.slot, self.parent, reference_tick, self.shred_version)
                .unwrap()
                .entries_to_shreds(
                    keypair,
                    entries,
                    is_slot_end,
                    None,
                    self.next_shred_index,
                    self.next_code_index,
                    true, // merkle_variant
                    &self.reed_solomon_cache,
                    process_stats,
                );
        process_stats.num_merkle_data_shreds += data_shreds.len();
        process_stats.num_merkle_coding_shreds += coding_shreds.len();
        if let Some(shred) = data_shreds.iter().max_by_key(|shred| shred.index()) {
            self.chained_merkle_root = shred.merkle_root().unwrap();
            self.next_shred_index = shred.index() + 1;
        };
        if self.next_shred_index > max_data_shreds_per_slot {
            return Err(StorageError::TooManyShreds.into());
        }
        if let Some(index) = coding_shreds.iter().map(Shred::index).max() {
            self.next_code_index = index + 1;
        };
        if self.next_code_index > max_code_shreds_per_slot {
            return Err(StorageError::TooManyShreds.into());
        }
        Ok((data_shreds, coding_shreds))
    }

    fn reset(
        &mut self,
        bank: Arc<Bank>,
        blockstore: &Blockstore,
        process_stats: &mut ProcessShredsStats,
    ) {
        let chained_merkle_root = if self.slot == bank.parent_slot() {
            self.chained_merkle_root
        } else {
            get_chained_merkle_root_from_parent(bank.slot(), bank.parent_slot(), blockstore)
                .unwrap_or_else(|err: Error| {
                    error!("Unknown chained Merkle root: {err:?}");
                    process_stats.err_unknown_chained_merkle_root += 1;
                    Hash::default()
                })
        };

        self.slot = bank.slot();
        self.parent = bank.parent_slot();
        self.chained_merkle_root = chained_merkle_root;
        self.next_shred_index = 0u32;
        self.next_code_index = 0u32;
        self.completed = false;
        self.num_batches = 0;
    }
}

fn get_chained_merkle_root_from_parent(
    slot: Slot,
    parent: Slot,
    blockstore: &Blockstore,
) -> Result<Hash> {
    if slot == parent {
        debug_assert_eq!(slot, 0u64);
        return Ok(Hash::default());
    }
    debug_assert!(parent < slot, "parent: {parent} >= slot: {slot}");
    let index = blockstore
        .meta(parent)?
        .ok_or(StorageError::UnknownSlotMeta(parent))?
        .last_index
        .ok_or(StorageError::UnknownLastIndex(parent))?;
    let shred = blockstore
        .get_data_shred(parent, index)?
        .ok_or(StorageError::ShredNotFound {
            slot: parent,
            index,
        })?;
    Ok(
        shred::layout::get_merkle_root(&shred).ok_or(StorageError::InvalidMerkleRoot {
            slot: parent,
            index,
        })?,
    )
}
