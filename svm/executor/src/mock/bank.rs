use {
    crate::{
        bank::{BankInfo, BankOperations},
        env::{DEPLOYMENT_EPOCH, DEPLOYMENT_SLOT},
    },
    solana_sdk::{
        account::{AccountSharedData, ReadableAccount, WritableAccount},
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        clock::{Clock, UnixTimestamp},
        feature_set::FeatureSet,
        native_loader,
        pubkey::Pubkey, sysvar::SysvarId,
    },
    solana_svm::transaction_processing_callback::TransactionProcessingCallback,
    std::{
        cell::RefCell,
        collections::HashMap,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    },
};

pub struct MockBankCallback {
    pub feature_set: Arc<FeatureSet>,
    pub account_shared_data: RefCell<HashMap<Pubkey, AccountSharedData>>,

    pub execution_slot: u64, // The execution slot must be greater than the deployment slot
    pub execution_epoch: u64, // The execution epoch must be greater than the deployment epoch
}

impl Default for MockBankCallback {
    fn default() -> Self {
        Self {
            feature_set: Default::default(),
            account_shared_data: Default::default(),
            execution_slot: 5,
            execution_epoch: 2,
        }
    }
}

impl TransactionProcessingCallback for MockBankCallback {
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        if let Some(data) = self.account_shared_data.borrow().get(account) {
            if data.lamports() == 0 {
                None
            } else {
                owners.iter().position(|entry| data.owner() == entry)
            }
        } else {
            None
        }
    }

    fn get_account_shared_data(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.account_shared_data.borrow().get(pubkey).cloned()
    }

    fn add_builtin_account(&self, name: &str, program_id: &Pubkey) {
        let account_data = native_loader::create_loadable_account_with_fields(name, (5000, 0));

        self.account_shared_data
            .borrow_mut()
            .insert(*program_id, account_data);
    }
}

impl BankOperations for MockBankCallback {
    fn insert_account(&mut self, key: Pubkey, data: AccountSharedData) {
        self.account_shared_data.borrow_mut().insert(key, data);
    }

    fn deploy_program(&mut self, mut buffer: Vec<u8>) -> Pubkey {
        let program_account = Pubkey::new_unique();
        let program_data_account = Pubkey::new_unique();
        let state = UpgradeableLoaderState::Program {
            programdata_address: program_data_account,
        };

        // The program account must have funds and hold the executable binary
        let mut account_data = AccountSharedData::default();
        account_data.set_data(bincode::serialize(&state).unwrap());
        account_data.set_lamports(25);
        account_data.set_owner(bpf_loader_upgradeable::id());
        self.insert_account(program_account, account_data);

        let mut account_data = AccountSharedData::default();
        let state = UpgradeableLoaderState::ProgramData {
            slot: DEPLOYMENT_SLOT,
            upgrade_authority_address: None,
        };
        let mut header = bincode::serialize(&state).unwrap();
        let mut complement = vec![
            0;
            std::cmp::max(
                0,
                UpgradeableLoaderState::size_of_programdata_metadata().saturating_sub(header.len())
            )
        ];
        header.append(&mut complement);
        header.append(&mut buffer);
        account_data.set_data(header);
        self.insert_account(program_data_account, account_data);

        program_account
    }

    fn set_clock(&mut self) {
        // We must fill in the sysvar cache entries
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;
        let clock = Clock {
            slot: DEPLOYMENT_SLOT,
            epoch_start_timestamp: time_now.saturating_sub(10) as UnixTimestamp,
            epoch: DEPLOYMENT_EPOCH,
            leader_schedule_epoch: DEPLOYMENT_EPOCH,
            unix_timestamp: time_now as UnixTimestamp,
        };

        let mut account_data = AccountSharedData::default();
        account_data.set_data(bincode::serialize(&clock).unwrap());
        self.insert_account(Clock::id(), account_data);
    }
}

impl BankInfo for MockBankCallback {
    fn last_blockhash(&self) -> solana_sdk::hash::Hash {
        Default::default()
    }

    fn execution_slot(&self) -> u64 {
        self.execution_slot
    }

    fn execution_epoch(&self) -> u64 {
        self.execution_epoch
    }
}

impl MockBankCallback {
    #[allow(dead_code)]
    pub fn override_feature_set(&mut self, new_set: FeatureSet) {
        self.feature_set = Arc::new(new_set)
    }
}
