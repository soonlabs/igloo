use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use solana_accounts_db::{
    accounts_db::{AccountShrinkThreshold, AccountsDbConfig},
    accounts_index::AccountSecondaryIndexes,
};
use solana_ledger::{
    blockstore_options::{BlockstoreRecoveryMode, LedgerColumnOptions},
    leader_schedule::FixedSchedule,
    use_snapshot_archives_at_startup::UseSnapshotArchivesAtStartup,
};
use solana_runtime::snapshot_config::SnapshotConfig;
use solana_sdk::{
    clock::Slot, genesis_config::GenesisConfig, hash::Hash, pubkey::Pubkey, signature::Keypair,
    signer::EncodableKey,
};
use solana_svm::runtime_config::RuntimeConfig;

use crate::{init::init_config, Result, RollupStorage};

pub const MAX_GENESIS_ARCHIVE_UNPACKED_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB

#[derive(Default, Clone)]
pub struct GlobalConfig {
    pub ledger_path: PathBuf,
    pub allow_default_genesis: bool,
    pub dev_mode: bool,
    pub keypairs: KeypairsConfig,
    pub storage: StorageConfig,
    pub genesis: GenesisConfig,
}

#[derive(Default, Clone)]
pub struct KeypairsConfig {
    pub validator_key_path: Option<PathBuf>,
    pub validator_keypair: Option<Arc<Keypair>>,
    pub mint_key_path: Option<PathBuf>,
    pub mint_keypair: Option<Arc<Keypair>>,
    pub voting_key_path: Option<PathBuf>,
    pub voting_keypair: Option<Arc<Keypair>>,
}

impl RollupStorage {
    pub fn keypairs(&self) -> &KeypairsConfig {
        &self.config.keypairs
    }
}

impl GlobalConfig {
    pub fn new(ledger_path: &Path) -> Result<Self> {
        let storage = init_config(ledger_path)?;

        Ok(Self {
            ledger_path: ledger_path.to_path_buf(),
            storage,
            ..Default::default()
        })
    }

    pub fn new_temp(ledger_path: &Path) -> Result<Self> {
        let storage = init_config(ledger_path)?;
        Ok(Self {
            ledger_path: ledger_path.to_path_buf(),
            storage,
            allow_default_genesis: true,
            ..Default::default()
        })
    }

    pub fn new_dev(ledger_path: &Path) -> Result<Self> {
        let storage = init_config(ledger_path)?;
        Ok(Self {
            ledger_path: ledger_path.to_path_buf(),
            storage,
            allow_default_genesis: true,
            dev_mode: true,
            ..Default::default()
        })
    }
}

#[derive(Clone)]
pub struct StorageConfig {
    pub halt_at_slot: Option<Slot>,
    pub expected_genesis_hash: Option<Hash>,
    pub account_paths: Vec<PathBuf>,
    pub snapshot_config: SnapshotConfig,
    pub wait_snapshot_complete: bool,
    pub wait_timeout: Duration,
    pub enforce_ulimit_nofile: bool,
    pub fixed_leader_schedule: Option<FixedSchedule>,
    pub new_hard_forks: Option<Vec<Slot>>,
    pub accounts_hash_interval_slots: u64,
    pub max_genesis_archive_unpacked_size: u64,
    pub wal_recovery_mode: Option<BlockstoreRecoveryMode>,
    pub run_verification: bool,
    pub debug_keys: Option<Arc<HashSet<Pubkey>>>,
    pub account_indexes: AccountSecondaryIndexes,
    pub accounts_db_config: Option<AccountsDbConfig>,
    pub accounts_db_test_hash_calculation: bool,
    pub accounts_db_skip_shrink: bool,
    pub accounts_db_force_initial_clean: bool,
    pub accounts_shrink_ratio: AccountShrinkThreshold,
    pub ledger_column_options: LedgerColumnOptions,
    pub runtime_config: RuntimeConfig,
    pub history_config: HistoryConfig,
    pub use_snapshot_archives_at_startup: UseSnapshotArchivesAtStartup,
}

#[derive(Clone)]
pub struct HistoryConfig {
    pub enable_transaction_history: bool,
    pub enable_extended_tx_metadata_storage: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            halt_at_slot: None,
            expected_genesis_hash: None,
            account_paths: Vec::new(),
            snapshot_config: SnapshotConfig::new_load_only(),
            wait_snapshot_complete: false,
            wait_timeout: Duration::from_secs(10),
            enforce_ulimit_nofile: true,
            fixed_leader_schedule: None,
            new_hard_forks: None,
            accounts_hash_interval_slots: u64::MAX,
            max_genesis_archive_unpacked_size: MAX_GENESIS_ARCHIVE_UNPACKED_SIZE,
            wal_recovery_mode: None,
            run_verification: true,
            debug_keys: None,
            account_indexes: AccountSecondaryIndexes::default(),
            accounts_db_test_hash_calculation: false,
            accounts_db_skip_shrink: false,
            accounts_db_force_initial_clean: false,
            accounts_shrink_ratio: AccountShrinkThreshold::default(),
            accounts_db_config: None,
            ledger_column_options: LedgerColumnOptions::default(),
            runtime_config: RuntimeConfig::default(),
            history_config: Default::default(),
            use_snapshot_archives_at_startup: UseSnapshotArchivesAtStartup::default(),
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enable_transaction_history: true,
            enable_extended_tx_metadata_storage: false,
        }
    }
}

impl KeypairsConfig {
    pub fn set_default_path(&mut self, base: &Path) {
        self.validator_key_path
            .get_or_insert(base.join("validator-identity.json"));
        self.mint_key_path
            .get_or_insert(base.join("validator-stake-account.json"));
        self.voting_key_path
            .get_or_insert(base.join("validator-vote-account.json"));
    }

    pub fn init(&mut self) -> crate::Result<()> {
        Self::try_init(
            &mut self.validator_keypair,
            self.validator_key_path.as_ref(),
        )?;
        Self::try_init(&mut self.mint_keypair, self.mint_key_path.as_ref())?;
        Self::try_init(&mut self.voting_keypair, self.voting_key_path.as_ref())?;
        Ok(())
    }

    fn try_init(source: &mut Option<Arc<Keypair>>, path: Option<&PathBuf>) -> crate::Result<()> {
        if source.is_some() {
            return Ok(());
        }
        if let Some(path) = path {
            let keypair = Self::init_from_file(path)?;
            *source = Some(Arc::new(keypair));
        }
        Ok(())
    }

    fn init_from_file(path: &Path) -> crate::Result<Keypair> {
        let keypair = Keypair::read_from_file(path).map_err(|e| {
            crate::Error::InitCommon(format!("failed to read validator keypair: {e}").to_string())
        })?;
        Ok(keypair)
    }
}
