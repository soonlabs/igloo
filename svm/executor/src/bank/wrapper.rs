use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use igloo_interface::l2::{
    bank::{BankInfo, BankOperations},
    executor::Init,
};
use solana_accounts_db::utils::create_accounts_run_and_snapshot_dirs;
use solana_ledger::genesis_utils::create_genesis_config;
use solana_runtime::{
    bank::{Bank, BankTestConfig},
    bank_forks::BankForks,
    installed_scheduler_pool::BankWithScheduler,
};
use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    account_utils::StateMut,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Slot,
    hash::{hashv, Hash},
    pubkey::Pubkey,
};
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;

use crate::error::{Error, Result};

use super::WrapperConfig;

#[derive(Clone)]
pub struct BankWrapper {
    bank: Arc<Bank>,
    cfg: WrapperConfig,

    pub validator_pubkey: Pubkey,
}

impl TransactionProcessingCallback for BankWrapper {
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        self.bank.account_matches_owners(account, owners)
    }

    fn get_account_shared_data(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.bank.get_account_shared_data(pubkey)
    }

    fn add_builtin_account(&self, name: &str, program_id: &Pubkey) {
        self.bank.add_builtin_account(name, program_id);
    }
}

impl BankOperations for BankWrapper {
    type Pubkey = Pubkey;
    type AccountSharedData = AccountSharedData;
    type Error = Error;

    fn insert_account(&mut self, key: Pubkey, data: AccountSharedData) -> Result<()> {
        self.bank.store_account(&key, &data);
        Ok(())
    }

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Result<Pubkey> {
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
                slot: self.cfg.previous_slot,
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

    fn set_clock(&mut self) -> Result<()> {
        // We do nothing here because there is a clock sysvar in the bank already
        Ok(())
    }

    fn bump(&mut self) -> Result<()> {
        // do nothing by default
        Ok(())
    }
}

impl BankInfo for BankWrapper {
    type Hash = Hash;
    type Pubkey = Pubkey;
    type Slot = Slot;
    type Error = Error;

    fn last_blockhash(&self) -> Hash {
        self.bank.last_blockhash()
    }

    fn execution_slot(&self) -> u64 {
        self.bank.slot()
    }

    fn collector_id(&self) -> std::result::Result<Self::Pubkey, Self::Error> {
        Ok(self.validator_pubkey)
    }
}

impl Init for BankWrapper {
    type Error = Error;

    type Config = super::WrapperConfig;

    fn init(cfg: &Self::Config) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Self::new(cfg))
    }
}

impl BankWrapper {
    pub fn new(cfg: &WrapperConfig) -> Self {
        let genesis = create_genesis_config(cfg.mint_lamports);
        let bank = Bank::new_for_tests(&genesis.genesis_config);
        let mut wrap = Self::new_from_bank(bank, cfg);
        wrap.validator_pubkey = genesis.validator_pubkey;
        wrap
    }

    pub fn new_from_bank(bank: Bank, cfg: &WrapperConfig) -> Self {
        let (bank, bank_forks) = bank.wrap_with_bank_forks_for_tests();
        goto_end_of_slot(bank.clone(), cfg);
        let bank = new_bank_from_parent_with_bank_forks(
            &bank_forks,
            bank,
            &Pubkey::default(),
            cfg.previous_slot,
        );
        let bank = new_bank_from_parent_with_bank_forks(
            &bank_forks,
            bank,
            &Pubkey::default(),
            cfg.latest_slot,
        );

        Self {
            cfg: cfg.clone(),
            bank,
            validator_pubkey: Default::default(),
        }
    }

    pub fn new_with_path(base_path: &Path, dir_count: u32, cfg: &WrapperConfig) -> Result<Self> {
        let paths = (0..dir_count)
            .map(|i| {
                let path = base_path.join(i.to_string());
                create_accounts_run_and_snapshot_dirs(&path).map(|(run_dir, _snapshot_dir)| run_dir)
            })
            .collect::<std::result::Result<Vec<_>, std::io::Error>>()?;
        let genesis = create_genesis_config(cfg.mint_lamports);
        let bank = Bank::new_with_paths_for_tests(
            &genesis.genesis_config,
            Default::default(),
            paths,
            BankTestConfig::default().secondary_indexes,
            Default::default(),
        );

        let mut wrap = Self::new_from_bank(bank, cfg);
        wrap.validator_pubkey = genesis.validator_pubkey;
        Ok(wrap)
    }
}

fn goto_end_of_slot(bank: Arc<Bank>, cfg: &WrapperConfig) {
    goto_end_of_slot_with_scheduler(&BankWithScheduler::new_without_scheduler(bank), cfg)
}

fn goto_end_of_slot_with_scheduler(bank: &BankWithScheduler, cfg: &WrapperConfig) {
    let mut tick_hash = bank.last_blockhash();
    loop {
        tick_hash = hashv(&[tick_hash.as_ref(), &[cfg.previous_slot as u8]]);
        bank.register_tick(&tick_hash);
        if tick_hash == bank.last_blockhash() {
            bank.freeze();
            return;
        }
    }
}

fn new_bank_from_parent_with_bank_forks(
    bank_forks: &RwLock<BankForks>,
    parent: Arc<Bank>,
    collector_id: &Pubkey,
    slot: Slot,
) -> Arc<Bank> {
    let bank = Bank::new_from_parent(parent, collector_id, slot);
    bank_forks
        .write()
        .unwrap()
        .insert(bank)
        .clone_without_scheduler()
}
