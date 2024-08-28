use process::EntriesProcessor;
use solana_entry::entry::{next_hash, Entry};
use solana_ledger::{blockstore_processor, shred::Shred};
use solana_sdk::{hash::Hash, transaction::VersionedTransaction};

use crate::{error::BankError, Error, Result, RollupStorage};

pub mod process;
pub mod txs;

const DEFAULT_NUM_HASHES: u64 = 2;

impl RollupStorage {
    pub(crate) fn aligne_blockstore_with_bank_forks(&self) -> Result<()> {
        blockstore_processor::process_blockstore_from_root(
            &self.blockstore,
            &self.bank_forks,
            &self.leader_schedule_cache,
            &self.process_options,
            None,
            None,
            None,
            &self.background_service.accounts_background_request_sender,
        )
        .map_err(|err| Error::InitCommon(format!("Failed to load ledger: {err:?}")))?;
        Ok(())
    }

    pub(crate) fn blockstore_save(&self, entries: Vec<Entry>) -> Result<()> {
        let (data_shreds, code_shreds) = self.transactions_to_shreds(entries)?;
        let _data_info =
            self.blockstore
                .insert_shreds(data_shreds, Some(&self.leader_schedule_cache), true)?;
        let _code_info =
            self.blockstore
                .insert_shreds(code_shreds, Some(&self.leader_schedule_cache), true)?;
        Ok(())
    }

    pub(crate) fn transactions_to_shreds(
        &self,
        entries: Vec<Entry>,
    ) -> Result<(Vec<Shred>, Vec<Shred>)> {
        let mut processor = EntriesProcessor::new(Default::default());

        processor.process(
            &entries,
            self.bank.clone(),
            &self.blockstore,
            self.bank.max_tick_height(), // use max tick height here
            self.config
                .keypairs
                .validator_keypair
                .as_ref()
                .ok_or(Error::KeypairsConfigMissingValidatorKeypair)?
                .as_ref(),
        )
    }

    pub(crate) fn transactions_to_entries(
        &self,
        transactions: Vec<VersionedTransaction>,
    ) -> Result<Vec<Entry>> {
        let mut start_hash = self
            .bank
            .parent()
            .ok_or(BankError::BankNotExists(self.bank.parent_slot()))?
            .last_blockhash();
        let entry = self.new_entry(&start_hash, DEFAULT_NUM_HASHES, transactions);
        start_hash = entry.hash;

        let mut entries = vec![entry];
        for _ in 0..self.config.genesis.ticks_per_slot {
            let entry = self.new_entry(&start_hash, DEFAULT_NUM_HASHES, vec![]);
            start_hash = entry.hash;
            entries.push(entry);
        }

        Ok(entries)
    }

    fn new_entry(
        &self,
        prev_hash: &Hash,
        mut num_hashes: u64,
        transactions: Vec<VersionedTransaction>,
    ) -> Entry {
        // If you passed in transactions, but passed in num_hashes == 0, then
        // next_hash will generate the next hash and set num_hashes == 1
        if num_hashes == 0 && !transactions.is_empty() {
            num_hashes = 1;
        }

        let hash = next_hash(prev_hash, num_hashes, &transactions);
        Entry {
            num_hashes,
            hash,
            transactions,
        }
    }
}
