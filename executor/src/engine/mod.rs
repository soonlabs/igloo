use crate::{
    defs::BlockPayload,
    processor::{bank::BankProcessor, Processor},
    Result,
};
use rollups_interface::l2::{bank::BankOperations, storage::StorageOperations};
use rollups_storage::{
    blockstore::txs::CommitBatch, config::GlobalConfig, ledger::SlotInfo, RollupStorage,
};
use rollups_validator::{
    settings::{Settings, Switchs},
    BankValidator, TransactionChecks,
};
use solana_sdk::clock::Slot;
use std::{borrow::Cow, path::Path};

pub mod storage;

pub struct Engine {
    storage: RollupStorage,
    finalized: Slot,
    validator_settings: Settings,
}

impl Engine {
    pub fn new(ledger_path: &Path) -> Result<Self> {
        let mut config = GlobalConfig::new(ledger_path)?;
        config.keypairs.set_default_path(ledger_path);
        Self::new_with_config(config)
    }

    pub fn new_for_test(ledger_path: &Path) -> Result<Self> {
        let config = GlobalConfig::new_temp(&ledger_path)?;
        Self::new_with_config(config)
    }

    pub fn new_with_config(config: GlobalConfig) -> Result<Self> {
        let settings = Settings {
            max_age: 150, // set default max_age from solana
            switchs: Switchs {
                tx_sanity_check: true,
                txs_conflict_check: true,
            },
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
            storage,
            finalized,
            validator_settings,
        })
    }

    pub async fn close(self) -> Result<()> {
        self.storage.close().await?;
        Ok(())
    }

    /// Check the block before processing
    pub fn check_block(&self, block: &BlockPayload, settins: Option<Switchs>) -> Result<()> {
        let switchs = settins.unwrap_or(self.validator_settings.switchs.clone());
        let validator = BankValidator::new(self.storage.current_bank(), Default::default());
        if switchs.tx_sanity_check {
            validator.transactions_sanity_check(&block.transactions)?;
        }
        if switchs.txs_conflict_check {
            validator.transactions_conflict_check(&block.transactions)?;
        }
        Ok(())
    }

    pub async fn new_block(&mut self, block: BlockPayload) -> Result<SlotInfo> {
        self.storage.bump()?;

        let processor =
            BankProcessor::new(self.storage.current_bank(), self.validator_settings.clone());
        let results = processor.process(Cow::Borrowed(&block.transactions))?;
        self.storage
            .commit(
                results.into(),
                CommitBatch::new(Cow::Borrowed(&block.transactions)),
            )
            .await?;

        let info = self.storage.get_slot_info(self.storage.current_height())?;
        info!("New block: {:?}", info);
        Ok(info)
    }

    pub fn confirm(&mut self, block: Slot) -> Result<()> {
        self.storage.confirm(block)?;
        Ok(())
    }

    pub fn reorg(&mut self, reset_to: Slot) -> Result<()> {
        self.storage.reorg(reset_to, Some(self.finalized))?;
        Ok(())
    }

    pub fn finalize(&mut self, block: Slot) -> Result<()> {
        self.storage.set_root(block, None)?;
        Ok(())
    }
}
