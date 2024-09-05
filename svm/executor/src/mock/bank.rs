use {
    crate::{
        env::{DEPLOYMENT_EPOCH, DEPLOYMENT_SLOT},
        error::Error,
    },
    igloo_interface::l2::{
        bank::{BankInfo, BankOperations},
        executor::{Config, Init},
    },
    solana_sdk::{
        account::{AccountSharedData, ReadableAccount, WritableAccount},
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        clock::{Clock, Slot, UnixTimestamp},
        feature_set::FeatureSet,
        native_loader,
        pubkey::Pubkey,
        sysvar::SysvarId,
    },
    solana_svm::transaction_processing_callback::TransactionProcessingCallback,
    std::{
        cell::RefCell,
        collections::HashMap,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    },
};

#[derive(Default)]
pub struct MockConfig {}

impl Config for MockConfig {}

pub struct MockBankCallback {
    pub feature_set: Arc<FeatureSet>,
    pub account_shared_data: RefCell<HashMap<Pubkey, AccountSharedData>>,

    pub execution_slot: u64, // The execution slot must be greater than the deployment slot
}

impl Default for MockBankCallback {
    fn default() -> Self {
        Self {
            feature_set: Default::default(),
            account_shared_data: Default::default(),
            execution_slot: 5,
        }
    }
}

impl Init for MockBankCallback {
    type Error = Error;

    type Config = MockConfig;

    fn init(_cfg: &Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Default::default())
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
    type Pubkey = Pubkey;

    type AccountSharedData = AccountSharedData;

    type Error = Error;

    fn insert_account(&mut self, key: Pubkey, data: AccountSharedData) -> Result<(), Self::Error> {
        self.account_shared_data.borrow_mut().insert(key, data);
        Ok(())
    }

    fn deploy_program(&mut self, mut buffer: Vec<u8>) -> Result<Pubkey, Self::Error> {
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
        self.insert_account(program_account, account_data)?;

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
        self.insert_account(program_data_account, account_data)?;

        Ok(program_account)
    }

    fn set_clock(&mut self) -> Result<(), Self::Error> {
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
        self.insert_account(Clock::id(), account_data)?;
        Ok(())
    }

    fn bump(&mut self) -> Result<(), Self::Error> {
        // do nothing by default
        Ok(())
    }
}

impl BankInfo for MockBankCallback {
    type Hash = solana_sdk::hash::Hash;

    type Pubkey = Pubkey;

    type Slot = Slot;

    type Error = Error;

    fn last_blockhash(&self) -> solana_sdk::hash::Hash {
        Default::default()
    }

    fn execution_slot(&self) -> u64 {
        self.execution_slot
    }

    fn collector_id(&self) -> Result<Self::Pubkey, Self::Error> {
        Ok(Default::default())
    }
}

impl MockBankCallback {
    #[allow(dead_code)]
    pub fn override_feature_set(&mut self, new_set: FeatureSet) {
        self.feature_set = Arc::new(new_set)
    }
}
