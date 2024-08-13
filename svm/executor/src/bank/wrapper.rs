use std::sync::{Arc, RwLock};

use solana_ledger::genesis_utils::{create_genesis_config, GenesisConfigInfo};
use solana_runtime::{
    bank::Bank, bank_forks::BankForks, installed_scheduler_pool::BankWithScheduler,
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

use super::{BankInfo, BankOperations};

const PREVIOUS_SLOT: Slot = 20;
const LATEST_SLOT: Slot = 30;

pub struct BankWrapper {
    bank: Arc<Bank>,
    _genesis: GenesisConfigInfo,
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
    fn insert_account(&mut self, key: Pubkey, data: AccountSharedData) {
        self.bank.store_account(&key, &data);
    }

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Pubkey {
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
                slot: PREVIOUS_SLOT,
                upgrade_authority_address: None,
            })
            .unwrap();
        programdata_account.data_as_mut_slice()[programdata_data_offset..].copy_from_slice(&buffer);
        programdata_account.set_rent_epoch(1);
        self.bank.store_account(&program_key, &program_account);
        self.bank
            .store_account(&programdata_key, &programdata_account);

        program_key
    }

    fn set_clock(&mut self) {
        // We do nothing here because there is a clock sysvar in the bank already
    }
}

impl BankInfo for BankWrapper {
    fn last_blockhash(&self) -> Hash {
        self.bank.last_blockhash()
    }

    fn execution_slot(&self) -> u64 {
        self.bank.slot()
    }

    fn execution_epoch(&self) -> u64 {
        self.bank.epoch()
    }
}

impl Default for BankWrapper {
    fn default() -> Self {
        let genesis = create_genesis_config(10_000);
        let bank = Bank::new_for_tests(&genesis.genesis_config);
        let (bank, bank_forks) = bank.wrap_with_bank_forks_for_tests();
        goto_end_of_slot(bank.clone());
        let bank = new_bank_from_parent_with_bank_forks(
            &bank_forks,
            bank,
            &Pubkey::default(),
            PREVIOUS_SLOT,
        );
        let bank = new_bank_from_parent_with_bank_forks(
            &bank_forks,
            bank,
            &Pubkey::default(),
            LATEST_SLOT,
        );

        Self {
            bank,
            _genesis: genesis,
        }
    }
}

fn goto_end_of_slot(bank: Arc<Bank>) {
    goto_end_of_slot_with_scheduler(&BankWithScheduler::new_without_scheduler(bank))
}

fn goto_end_of_slot_with_scheduler(bank: &BankWithScheduler) {
    let mut tick_hash = bank.last_blockhash();
    loop {
        tick_hash = hashv(&[tick_hash.as_ref(), &[PREVIOUS_SLOT as u8]]);
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
