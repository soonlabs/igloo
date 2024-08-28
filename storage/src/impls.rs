use rollups_interface::l2::{
    bank::{BankInfo, BankOperations},
    executor::Init,
    storage::{StorageOperations, TransactionSet, TransactionsResult},
};
use solana_gossip::cluster_info::ClusterInfo;
use solana_ledger::{
    blockstore::Blockstore, blockstore_processor::ProcessOptions,
    leader_schedule_cache::LeaderScheduleCache,
};
use solana_runtime::{bank::Bank, bank_forks::BankForks};
use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    account_utils::StateMut,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Slot,
    pubkey::Pubkey,
    signer::Signer,
};
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

use crate::{
    background::StorageBackground, blockstore::txs::CommitBatch, config::GlobalConfig,
    error::BankError, execution::TransactionsResultWrapper, Error,
};

pub struct RollupStorage {
    pub(crate) bank: Arc<Bank>,
    pub(crate) bank_forks: Arc<RwLock<BankForks>>,

    pub(crate) cluster_info: Arc<ClusterInfo>,
    pub(crate) config: GlobalConfig,
    pub(crate) blockstore: Arc<Blockstore>,
    pub(crate) background_service: StorageBackground,
    pub(crate) leader_schedule_cache: Arc<LeaderScheduleCache>,
    pub(crate) process_options: ProcessOptions,
    pub(crate) exit: Arc<AtomicBool>,
}

impl Init for RollupStorage {
    type Error = crate::Error;
    type Config = GlobalConfig;

    fn init(cfg: &Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Self::new(cfg.clone())
    }
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

impl BankOperations for RollupStorage {
    type Pubkey = Pubkey;

    type AccountSharedData = AccountSharedData;

    type Error = Error;

    fn insert_account(
        &mut self,
        key: Self::Pubkey,
        data: Self::AccountSharedData,
    ) -> Result<(), Self::Error> {
        if self.config.dev_mode {
            self.bank.store_account(&key, &data);
            Ok(())
        } else {
            Err(BankError::InvalidOperation("Capitalization check not passed".to_string()).into())
        }
    }

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Result<Self::Pubkey, Self::Error> {
        let program_key = solana_sdk::pubkey::new_rand();
        let programdata_key = solana_sdk::pubkey::new_rand();

        let mut program_account = AccountSharedData::new_data(
            40,
            &UpgradeableLoaderState::Program {
                programdata_address: programdata_key,
            },
            &bpf_loader_upgradeable::id(),
        )
        .unwrap();
        program_account.set_executable(true);
        program_account.set_rent_epoch(1);
        let programdata_data_offset = UpgradeableLoaderState::size_of_programdata_metadata();
        let mut programdata_account = AccountSharedData::new(
            40,
            programdata_data_offset + buffer.len(),
            &bpf_loader_upgradeable::id(),
        );
        programdata_account
            .set_state(&UpgradeableLoaderState::ProgramData {
                slot: self.bank.parent_slot(),
                upgrade_authority_address: None,
            })
            .unwrap();
        programdata_account.data_as_mut_slice()[programdata_data_offset..].copy_from_slice(&buffer);
        programdata_account.set_rent_epoch(1);
        self.bank.store_account(&program_key, &program_account);
        self.bank
            .store_account(&programdata_key, &programdata_account);

        Ok(program_key)
    }

    fn set_clock(&mut self) -> Result<(), Self::Error> {
        // We do nothing here because there is a clock sysvar in the bank already
        Ok(())
    }

    fn bump(&mut self) -> Result<(), Self::Error> {
        let slot = self.bank_forks.read().unwrap().highest_slot();
        self.bump_slot(slot + 1)?;
        Ok(())
    }
}

impl BankInfo for RollupStorage {
    type Hash = solana_sdk::hash::Hash;

    type Pubkey = Pubkey;

    type Slot = Slot;

    type Error = Error;

    fn last_blockhash(&self) -> Self::Hash {
        self.bank.last_blockhash()
    }

    fn execution_slot(&self) -> Self::Slot {
        self.bank.slot()
    }

    fn collector_id(&self) -> Result<Self::Pubkey, Self::Error> {
        Ok(self
            .config
            .keypairs
            .validator_keypair
            .as_ref()
            .ok_or(Error::KeypairsConfigMissingValidatorKeypair)?
            .pubkey())
    }
}

impl StorageOperations for RollupStorage {
    type Error = Error;
    type TxsResult = TransactionsResultWrapper;
    type TransactionSet<'a> = CommitBatch<'a>;

    async fn commit<'a>(
        &mut self,
        result: Self::TxsResult,
        origin: &Self::TransactionSet<'a>,
    ) -> Result<(), Self::Error> {
        // TODO: make commit async
        let executed_txs = result.success_txs(origin.transactions());
        let entries = self.transactions_to_entries(executed_txs)?;

        self.bank_commit(result, origin, &entries)?;
        self.blockstore_save(entries)?;

        Ok(())
    }

    async fn force_save(&mut self) -> Result<(), Self::Error> {
        self.bank.freeze();
        self.set_root(self.bank.slot(), None)?;
        Ok(())
    }

    async fn close(self) -> Result<(), Self::Error> {
        self.exit.store(true, std::sync::atomic::Ordering::Relaxed);
        self.blockstore.drop_signal();
        self.join();
        Ok(())
    }
}

impl RollupStorage {
    pub(crate) fn join(self) {
        drop(self.bank);
        drop(self.bank_forks);
        self.background_service.join();
    }

    pub fn cluster_info(&self) -> Arc<ClusterInfo> {
        self.cluster_info.clone()
    }
}
