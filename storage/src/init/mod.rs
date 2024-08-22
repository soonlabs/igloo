use crate::{
    background::StorageBackground,
    config::{GlobalConfig, StorageConfig},
    sig_hub::SignalHub,
    Error, Result, RollupStorage,
};
use default::default_genesis_config;
use solana_accounts_db::{
    accounts_db::AccountsDbConfig,
    hardened_unpack::open_genesis_config,
    utils::{create_all_accounts_run_and_snapshot_dirs, create_and_canonicalize_directories},
};
use solana_gossip::{cluster_info::ClusterInfo, contact_info::ContactInfo};
use solana_ledger::{
    bank_forks_utils, blockstore::Blockstore, blockstore_options::BlockstoreOptions,
    blockstore_processor, leader_schedule_cache::LeaderScheduleCache,
};
use solana_runtime::{
    accounts_background_service::AccountsBackgroundService, bank_forks::BankForks,
    snapshot_hash::StartingSnapshotHashes,
};
use solana_sdk::{
    genesis_config::GenesisConfig, signature::Keypair, signer::Signer, timing::timestamp,
};
use solana_streamer::socket::SocketAddrSpace;
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, RwLock},
};

pub mod default;

pub const MAX_REPLAY_WAKE_UP_SIGNALS: usize = 1;

impl RollupStorage {
    pub fn new(mut config: GlobalConfig) -> Result<Self> {
        let exit = Arc::new(AtomicBool::new(false));
        let (
            genesis_config,
            bank_forks,
            blockstore,
            leader_schedule_cache,
            starting_snapshot_hashes,
            process_options,
            mut hub,
        ) = load_blockstore(&config, exit.clone(), SignalHub::default())?;
        config.genesis = genesis_config;

        let keypair = Arc::new(Keypair::new());
        let cluster_info = create_cluster_info(keypair.clone());

        let background_service = StorageBackground::new(
            bank_forks.clone(),
            &config.storage,
            &mut hub,
            exit.clone(),
            cluster_info.clone(),
            starting_snapshot_hashes,
        )?;

        let bank = bank_forks.read().unwrap().working_bank();
        Ok(Self {
            exit,
            config,
            bank_forks,
            bank,
            blockstore,
            background_service,
            leader_schedule_cache: Arc::new(leader_schedule_cache),
            keypair,
            cluster_info,
            process_options,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        self.aligne_blockstore_with_bank_forks()?;
        Ok(())
    }

    pub fn allow_init_from_scratch(&self) -> bool {
        self.config.allow_default_genesis
    }
}

pub(crate) fn init_config(ledger_path: &Path) -> Result<StorageConfig> {
    let ledger_path = create_and_canonicalize_directories([&ledger_path])
        .map_err(|e| Error::InitConfigFailed(e.to_string()))?
        .pop()
        .unwrap();

    let accounts_db_config = AccountsDbConfig {
        base_working_path: Some(ledger_path.clone()),
        ..AccountsDbConfig::default()
    };

    let account_paths: Vec<PathBuf> = vec![ledger_path.join("accounts")];
    let account_paths = create_and_canonicalize_directories(account_paths)
        .map_err(|err| Error::InitConfigFailed(format!("Unable to access account path: {err}")))?;

    let (account_run_paths, _account_snapshot_paths) =
        create_all_accounts_run_and_snapshot_dirs(&account_paths).map_err(|err| {
            Error::InitConfigFailed(format!("Create all accounts run and snapshot dirs: {err}"))
        })?;

    Ok(StorageConfig {
        accounts_db_config: Some(accounts_db_config),
        account_paths: account_run_paths,
        ..Default::default()
    })
}

fn load_blockstore(
    cfg: &GlobalConfig,
    exit: Arc<AtomicBool>,
    mut hub: SignalHub,
) -> Result<(
    GenesisConfig,
    Arc<RwLock<BankForks>>,
    Arc<Blockstore>,
    LeaderScheduleCache,
    Option<StartingSnapshotHashes>,
    blockstore_processor::ProcessOptions,
    SignalHub,
)> {
    let config = &cfg.storage;
    let ledger_path = &cfg.ledger_path;
    info!("loading ledger from {:?}...", ledger_path);
    let genesis_config =
        match open_genesis_config(ledger_path, config.max_genesis_archive_unpacked_size) {
            Ok(genesis_config) => Ok(genesis_config),
            Err(err) => {
                if cfg.allow_default_genesis {
                    Ok(default_genesis_config(&cfg.ledger_path)?)
                } else {
                    Err(Error::LoadBlockstoreFailed(format!(
                        "Failed to open genesis config: {err}"
                    )))
                }
            }
        }?;

    let genesis_hash = genesis_config.hash();
    info!("genesis hash: {}", genesis_hash);

    if let Some(expected_genesis_hash) = config.expected_genesis_hash {
        if genesis_hash != expected_genesis_hash {
            return Err(Error::LoadBlockstoreFailed( format!(
                "genesis hash mismatch: hash={genesis_hash} expected={expected_genesis_hash}. Delete the ledger directory to continue: {ledger_path:?}",
            )));
        }
    }

    let blockstore =
        Blockstore::open_with_options(ledger_path, blockstore_options_from_config(config))
            .map_err(|err| {
                Error::LoadBlockstoreFailed(format!("Failed to open Blockstore: {err:?}"))
            })?;

    let (ledger_signal_sender, ledger_signal_receiver) =
        crossbeam_channel::bounded(MAX_REPLAY_WAKE_UP_SIGNALS);
    hub.ledger_signal_receiver = Some(ledger_signal_receiver);
    blockstore.add_new_shred_signal(ledger_signal_sender);

    let blockstore = Arc::new(blockstore);
    let halt_at_slot = config
        .halt_at_slot
        .or_else(|| blockstore.highest_slot().unwrap_or(None));

    let process_options = blockstore_processor::ProcessOptions {
        run_verification: config.run_verification,
        halt_at_slot,
        new_hard_forks: config.new_hard_forks.clone(),
        debug_keys: config.debug_keys.clone(),
        account_indexes: config.account_indexes.clone(),
        accounts_db_config: config.accounts_db_config.clone(),
        shrink_ratio: config.accounts_shrink_ratio,
        accounts_db_test_hash_calculation: config.accounts_db_test_hash_calculation,
        accounts_db_skip_shrink: config.accounts_db_skip_shrink,
        accounts_db_force_initial_clean: config.accounts_db_force_initial_clean,
        runtime_config: config.runtime_config.clone(),
        use_snapshot_archives_at_startup: config.use_snapshot_archives_at_startup,
        ..blockstore_processor::ProcessOptions::default()
    };

    let (bank_forks, mut leader_schedule_cache, starting_snapshot_hashes) =
        bank_forks_utils::load_bank_forks(
            &genesis_config,
            &blockstore,
            config.account_paths.clone(),
            Some(&config.snapshot_config),
            &process_options,
            None,
            None,
            None,
            exit,
        )
        .map_err(|err| Error::LoadBlockstoreFailed(err.to_string()))?;

    // Before replay starts, set the callbacks in each of the banks in BankForks so that
    // all dropped banks come through the `pruned_banks_receiver` channel. This way all bank
    // drop behavior can be safely synchronized with any other ongoing accounts activity like
    // cache flush, clean, shrink, as long as the same thread performing those activities also
    // is processing the dropped banks from the `pruned_banks_receiver` channel.
    hub.pruned_banks_receiver = Some(AccountsBackgroundService::setup_bank_drop_callback(
        bank_forks.clone(),
    ));

    leader_schedule_cache.set_fixed_leader_schedule(config.fixed_leader_schedule.clone());
    {
        let mut bank_forks = bank_forks.write().unwrap();
        bank_forks.set_snapshot_config(Some(config.snapshot_config.clone()));
        bank_forks.set_accounts_hash_interval_slots(config.accounts_hash_interval_slots);
    }

    Ok((
        genesis_config,
        bank_forks,
        blockstore,
        leader_schedule_cache,
        starting_snapshot_hashes,
        process_options,
        hub,
    ))
}

fn blockstore_options_from_config(config: &StorageConfig) -> BlockstoreOptions {
    BlockstoreOptions {
        recovery_mode: config.wal_recovery_mode.clone(),
        column_options: config.ledger_column_options.clone(),
        enforce_ulimit_nofile: config.enforce_ulimit_nofile,
        ..BlockstoreOptions::default()
    }
}

fn create_cluster_info(keypair: Arc<Keypair>) -> Arc<ClusterInfo> {
    let contract_info = ContactInfo::new(keypair.pubkey(), timestamp(), Default::default());
    let cluster_info = ClusterInfo::new(contract_info, keypair, SocketAddrSpace::new(false));
    // TDOO: init cluster later
    // cluster_info.set_contact_debug_interval(..);
    // cluster_info.set_entrypoints(..);
    // cluster_info.restore_contact_info(..);

    Arc::new(cluster_info)
}
