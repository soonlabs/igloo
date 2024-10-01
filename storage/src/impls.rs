use crate::{
    background::StorageBackground, blockstore::txs::CommitBatch, config::GlobalConfig,
    error::BankError, execution::TransactionsResultWrapper, history::StorageHistoryServices,
    BankInfo, Error, Result,
};
use solana_gossip::cluster_info::ClusterInfo;
use solana_ledger::{
    blockstore::Blockstore, blockstore_processor::ProcessOptions,
    leader_schedule_cache::LeaderScheduleCache,
};
use solana_runtime::{bank::Bank, bank_forks::BankForks};
use solana_sdk::{account::AccountSharedData, clock::Slot, pubkey::Pubkey, signer::Signer};
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

pub struct RollupStorage {
    pub(crate) bank: Arc<Bank>,
    pub(crate) bank_forks: Arc<RwLock<BankForks>>,

    pub(crate) cluster_info: Arc<ClusterInfo>,
    pub(crate) config: GlobalConfig,
    pub(crate) blockstore: Arc<Blockstore>,
    pub(crate) background_service: StorageBackground,
    pub(crate) history_services: StorageHistoryServices,
    pub(crate) leader_schedule_cache: Arc<LeaderScheduleCache>,
    pub(crate) process_options: ProcessOptions,
    pub(crate) exit: Arc<AtomicBool>,
}

impl TransactionProcessingCallback for RollupStorage {
    fn account_matches_owners(
        &self,
        account: &solana_sdk::pubkey::Pubkey,
        owners: &[solana_sdk::pubkey::Pubkey],
    ) -> Option<usize> {
        self.bank.account_matches_owners(account, owners)
    }

    fn get_account_shared_data(
        &self,
        pubkey: &solana_sdk::pubkey::Pubkey,
    ) -> Option<solana_sdk::account::AccountSharedData> {
        self.bank.get_account_shared_data(pubkey)
    }

    fn add_builtin_account(&self, name: &str, program_id: &Pubkey) {
        self.bank.add_builtin_account(name, program_id)
    }
}

impl BankInfo for RollupStorage {
    type Hash = solana_sdk::hash::Hash;

    type Pubkey = Pubkey;

    type Slot = Slot;

    type Error = Error;

    fn last_blockhash(&self, slot: Option<Slot>) -> std::result::Result<Self::Hash, Self::Error> {
        Ok(if let Some(slot) = slot {
            self.bank_forks
                .read()
                .unwrap()
                .get(slot)
                .ok_or(BankError::BankNotExists(slot))?
                .last_blockhash()
        } else {
            self.bank.last_blockhash()
        })
    }

    fn execution_slot(&self) -> Self::Slot {
        self.bank.slot()
    }

    fn collector_id(&self) -> std::result::Result<Self::Pubkey, Self::Error> {
        Ok(self
            .config
            .keypairs
            .validator_keypair
            .as_ref()
            .ok_or(Error::KeypairsConfigMissingValidatorKeypair)?
            .pubkey())
    }
}

impl RollupStorage {
    pub(crate) fn join(self) {
        drop(self.bank);
        drop(self.bank_forks);
        self.background_service.join();
    }

    pub fn config(&self) -> &GlobalConfig {
        &self.config
    }

    pub fn bank_forks(&self) -> Arc<RwLock<BankForks>> {
        self.bank_forks.clone()
    }

    pub fn blockstore(&self) -> Arc<Blockstore> {
        self.blockstore.clone()
    }

    pub fn history_services(&self) -> &StorageHistoryServices {
        &self.history_services
    }

    pub fn cluster_info(&self) -> Arc<ClusterInfo> {
        self.cluster_info.clone()
    }

    pub async fn commit<'a>(
        &mut self,
        result: Vec<TransactionsResultWrapper>,
        origin: Vec<CommitBatch<'a>>,
    ) -> Result<()> {
        self.commit_block(result, origin)?;
        Ok(())
    }

    pub async fn force_save(&mut self) -> Result<()> {
        self.bank.freeze();
        self.set_root(self.bank.slot(), None)?;
        Ok(())
    }

    pub async fn close(self) -> Result<()> {
        self.try_wait_snapshot_complete().await;
        self.exit.store(true, std::sync::atomic::Ordering::Relaxed);
        self.blockstore.drop_signal();
        self.join();
        Ok(())
    }

    pub fn bump(&mut self) -> Result<()> {
        let slot = self.bank_forks.read().unwrap().highest_slot();
        self.bump_slot(slot + 1)?;
        Ok(())
    }

    pub fn insert_account(&mut self, key: Pubkey, data: AccountSharedData) -> Result<()> {
        if self.config.dev_mode {
            self.bank.store_account(&key, &data);
            Ok(())
        } else {
            Err(BankError::InvalidOperation("Capitalization check not passed".to_string()).into())
        }
    }

    pub fn get_bank(&self, slot: Slot) -> Result<Arc<Bank>> {
        self.bank_forks
            .read()
            .unwrap()
            .get(slot)
            .ok_or(BankError::BankNotExists(slot).into())
    }
}
