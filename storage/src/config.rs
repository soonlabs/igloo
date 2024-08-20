use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use rollups_interface::l2::executor::Config;
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
use solana_sdk::{clock::Slot, genesis_config::GenesisConfig, hash::Hash, pubkey::Pubkey};
use solana_svm::runtime_config::RuntimeConfig;

use crate::{init::init_config, Error, Result};

pub const MAX_GENESIS_ARCHIVE_UNPACKED_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB

#[derive(Default, Clone)]
pub struct GlobalConfig {
    pub collector_id: Pubkey,
    pub ledger_path: PathBuf,
    pub allow_default_genesis: bool,
    pub storage: StorageConfig,
    pub genesis: GenesisConfig,
}

impl Config for GlobalConfig {}

impl GlobalConfig {
    pub fn new(ledger_path: &Path) -> Result<Self> {
        let storage = init_config(ledger_path)?;

        Ok(Self {
            ledger_path: ledger_path.to_path_buf(),
            storage,
            ..Default::default()
        })
    }

    pub fn new_temp() -> Result<Self> {
        let ledger_path = tempfile::tempdir()
            .map_err(|e| Error::InitCommon(e.to_string()))?
            .into_path();
        let storage = init_config(&ledger_path)?;
        Ok(Self {
            ledger_path,
            storage,
            allow_default_genesis: true,
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
    pub use_snapshot_archives_at_startup: UseSnapshotArchivesAtStartup,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            halt_at_slot: None,
            expected_genesis_hash: None,
            account_paths: Vec::new(),
            snapshot_config: SnapshotConfig::new_load_only(),
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
            use_snapshot_archives_at_startup: UseSnapshotArchivesAtStartup::default(),
        }
    }
}
