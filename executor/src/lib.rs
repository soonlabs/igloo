use crate::processor::TransactionProcessor;
use async_trait::async_trait;
use igloo_storage::execution::TransactionsResultWrapper;
use igloo_storage::{
    blockstore::txs::CommitBatch, config::GlobalConfig, ledger::SlotInfo, RollupStorage,
};
use igloo_verifier::{
    settings::{Settings, Switchs},
    BankVerifier,
};
use solana_sdk::{clock::Slot, fee::FeeStructure, transaction::SanitizedTransaction};
use std::{
    borrow::Cow,
    fmt::{Debug, Display},
    path::Path,
};

pub mod error;
pub mod processor;
#[cfg(test)]
mod tests;

pub use error::{Error, Result};

#[async_trait]
pub trait StreamOperator {
    type Error: Display;
    async fn next_batch(&self) -> std::result::Result<Vec<SanitizedTransactions>, Self::Error>;
}

pub type SanitizedTransactions = Vec<SanitizedTransaction>;

#[derive(Clone, Default)]
pub struct BlockPayload {
    pub transactions: Vec<SanitizedTransactions>,
}

#[derive(Default)]
pub struct Executor {
    storage: Option<RollupStorage>,
    finalized: Slot,
    validator_settings: Settings,
}

impl Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("finalized", &self.finalized)
            .field("validator_settings", &self.validator_settings)
            .finish()
    }
}

impl Executor {
    pub fn new_for_test(ledger_path: &Path) -> Result<Self> {
        let config = GlobalConfig::new_temp(ledger_path)?;
        Self::new_with_config(config)
    }

    pub fn new_with_config(config: GlobalConfig) -> Result<Self> {
        let settings = Settings {
            max_age: 150, // set default max_age from solana
            switchs: Switchs {
                tx_sanity_check: true,
                txs_conflict_check: true,
            },
            fee_structure: FeeStructure::new(0.0000005, 0.0, vec![(1_400_000, 0.0)]),
        };
        Self::new_with_validator_settings(config, settings)
    }

    pub fn new_with_validator_settings(
        config: GlobalConfig,
        validator_settings: Settings,
    ) -> Result<Self> {
        let mut storage = RollupStorage::new(config)?;
        storage.init()?;

        let finalized = storage.get_root();
        Ok(Self {
            storage: Some(storage),
            finalized,
            validator_settings,
        })
    }

    pub async fn close(self) -> Result<()> {
        self.storage.ok_or(Error::StorageIsNone)?.close().await?;
        Ok(())
    }

    /// Check the block before processing
    pub fn check_block(&self, block: &BlockPayload, settins: Option<Switchs>) -> Result<()> {
        let switchs = settins.unwrap_or(self.validator_settings.switchs.clone());
        let validator = BankVerifier::new(self.storage()?.current_bank(), Default::default());
        if switchs.tx_sanity_check {
            for txs in block.transactions.iter() {
                validator.transactions_sanity_check(txs)?;
            }
        }
        if switchs.txs_conflict_check {
            for txs in block.transactions.iter() {
                validator.transactions_conflict_check(txs)?;
            }
        }
        Ok(())
    }

    pub async fn new_block(&mut self, block: BlockPayload) -> Result<SlotInfo> {
        self.storage_mut()?.bump()?;

        let processor = TransactionProcessor::new(
            self.storage()?.current_bank(),
            self.validator_settings.clone(),
        );
        let mut results = vec![];
        let mut origin_txs = vec![];
        for transactions in block.transactions.iter() {
            let result: TransactionsResultWrapper =
                processor.process(Cow::Borrowed(transactions))?.into();
            results.push(result);
            origin_txs.push(CommitBatch::new(Cow::Borrowed(transactions)));
        }
        self.storage_mut()?.commit(results, origin_txs).await?;

        let current = self.storage()?.current_height();
        self.storage_mut()?.confirm(current)?;
        let info = self
            .storage()?
            .get_slot_info(self.storage()?.current_height())?;
        Ok(info)
    }

    pub fn reorg(&mut self, reset_to: Slot) -> Result<()> {
        let finalized = Some(self.finalized);
        self.storage_mut()?.reorg(reset_to, finalized)?;
        Ok(())
    }

    pub fn finalize(&mut self, block: Slot) -> Result<()> {
        self.storage_mut()?.set_root(block, None)?;
        Ok(())
    }

    pub fn storage(&self) -> Result<&RollupStorage> {
        self.storage.as_ref().ok_or(Error::StorageIsNone)
    }

    pub fn storage_mut(&mut self) -> Result<&mut RollupStorage> {
        self.storage.as_mut().ok_or(Error::StorageIsNone)
    }
}

impl BlockPayload {
    pub fn new(transactions: Vec<SanitizedTransaction>) -> Self {
        Self {
            transactions: vec![transactions],
        }
    }

    pub fn new_with_batches(batches: Vec<SanitizedTransactions>) -> Self {
        Self {
            transactions: batches,
        }
    }

    /// Extends the block with stream
    pub async fn extend_with<S: StreamOperator>(&mut self, stream: &S) -> Result<()> {
        self.transactions.extend(
            stream
                .next_batch()
                .await
                .map_err(|e| Error::FetchStreamBatchError(e.to_string()))?,
        );
        Ok(())
    }
}
