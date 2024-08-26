use process::EntriesProcessor;
use rollups_interface::l2::storage::TransactionsResult;
use solana_entry::entry::Entry;
use solana_ledger::{blockstore_processor, shred::Shred};
use solana_sdk::transaction::{SanitizedTransaction, VersionedTransaction};

use crate::{execution::TransactionsResultWrapper, Error, Result, RollupStorage};

pub mod process;
pub mod txs;

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

    pub(crate) fn blockstore_save(
        &self,
        result: &TransactionsResultWrapper,
        extras: &[SanitizedTransaction],
    ) -> Result<()> {
        let executed_txs = result.success_txs(extras);
        let (data_shreds, code_shreds) = self.transactions_to_shreds(executed_txs)?;
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
        txs: Vec<VersionedTransaction>,
    ) -> Result<(Vec<Shred>, Vec<Shred>)> {
        let entries = self.transactions_to_entries(txs);
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

    fn transactions_to_entries(&self, transactions: Vec<VersionedTransaction>) -> Vec<Entry> {
        let entry = Entry {
            transactions,
            ..Default::default()
        };
        vec![entry]
    }
}
