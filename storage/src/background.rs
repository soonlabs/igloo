use std::{
    sync::{atomic::AtomicBool, Arc, RwLock},
    time::Duration,
};

use crossbeam_channel::unbounded;
use solana_core::{
    accounts_hash_verifier::AccountsHashVerifier,
    snapshot_packager_service::SnapshotPackagerService,
};
use solana_gossip::cluster_info::ClusterInfo;
use solana_runtime::{
    accounts_background_service::{
        AbsRequestHandlers, AbsRequestSender, AccountsBackgroundService, PrunedBanksRequestHandler,
        SnapshotRequestHandler,
    },
    bank_forks::BankForks,
    snapshot_config::SnapshotConfig,
    snapshot_hash::StartingSnapshotHashes,
    snapshot_utils,
};
use tokio::time::sleep;

use crate::{config::StorageConfig, sig_hub::SignalHub, Error, Result, RollupStorage};

#[allow(dead_code)]
pub struct StorageBackground {
    pub(crate) accounts_background_service: AccountsBackgroundService,
    pub(crate) accounts_background_request_sender: AbsRequestSender,
    pub(crate) accounts_hash_verifier: AccountsHashVerifier,
    pub(crate) snapshot_packager_service: SnapshotPackagerService,
}

impl RollupStorage {
    pub async fn try_wait_snapshot_complete(&self) {
        let storage_cfg = &self.config.storage;
        if !storage_cfg.snapshot_config.should_generate_snapshots()
            || !storage_cfg.wait_snapshot_complete
        {
            return;
        }

        let full_latest = self.wait_full_snapshot(&storage_cfg.snapshot_config).await;
        if let Some(full_latest) = full_latest {
            self.wait_incremental_snapshot(&storage_cfg.snapshot_config, full_latest)
                .await;
        }
    }

    async fn wait_full_snapshot(&self, snapshot_config: &SnapshotConfig) -> Option<u64> {
        let expect_slot = self.get_latest_snapshot(
            self.current_height(),
            snapshot_config.full_snapshot_archive_interval_slots,
        );
        if let Some(slot) = expect_slot {
            while snapshot_utils::get_highest_full_snapshot_archive_slot(
                &snapshot_config.full_snapshot_archives_dir,
            ) != Some(slot)
            {
                debug!("Waiting for full snapshot {slot}");
                sleep(Duration::from_millis(20)).await;
            }
        }

        expect_slot
    }

    async fn wait_incremental_snapshot(&self, snapshot_config: &SnapshotConfig, full_latest: u64) {
        let expect_slot = self.get_latest_snapshot(
            self.current_height(),
            snapshot_config.incremental_snapshot_archive_interval_slots,
        );

        if let Some(slot) = expect_slot {
            while snapshot_utils::get_highest_incremental_snapshot_archive_slot(
                &snapshot_config.incremental_snapshot_archives_dir,
                full_latest,
            ) != Some(slot)
            {
                debug!("Waiting for incremental snapshot {slot}");
                sleep(Duration::from_millis(20)).await;
            }
        }
    }

    fn get_latest_snapshot(&self, highest: u64, inteval: u64) -> Option<u64> {
        let reminder = highest % inteval;
        if reminder >= highest {
            None
        } else {
            Some(highest - reminder)
        }
    }
}

impl StorageBackground {
    pub fn new(
        bank_forks: Arc<RwLock<BankForks>>,
        config: &StorageConfig,
        hub: &mut SignalHub,
        exit: Arc<AtomicBool>,
        cluster_info: Arc<ClusterInfo>,
        starting_snapshot_hashes: Option<StartingSnapshotHashes>,
    ) -> Result<Self> {
        let (snapshot_package_sender, snapshot_package_receiver) = crossbeam_channel::unbounded();
        let snapshot_packager_service = SnapshotPackagerService::new(
            snapshot_package_sender.clone(),
            snapshot_package_receiver,
            starting_snapshot_hashes,
            exit.clone(),
            cluster_info.clone(),
            config.snapshot_config.clone(),
            false,
        );

        let (accounts_package_sender, accounts_package_receiver) = crossbeam_channel::unbounded();
        let accounts_hash_verifier = AccountsHashVerifier::new(
            accounts_package_sender.clone(),
            accounts_package_receiver,
            Some(snapshot_package_sender),
            exit.clone(),
            config.snapshot_config.clone(),
        );

        let (snapshot_request_sender, snapshot_request_receiver) = unbounded();
        let accounts_background_request_sender =
            AbsRequestSender::new(snapshot_request_sender.clone());
        let snapshot_request_handler = SnapshotRequestHandler {
            snapshot_config: config.snapshot_config.clone(),
            snapshot_request_sender,
            snapshot_request_receiver,
            accounts_package_sender,
        };
        let pruned_banks_request_handler = PrunedBanksRequestHandler {
            pruned_banks_receiver: hub.pruned_banks_receiver.take().ok_or(Error::InitCommon(
                "pruned banks receiver is None".to_string(),
            ))?,
        };

        let last_full_snapshot_slot = starting_snapshot_hashes.map(|x| x.full.0 .0);
        let accounts_background_service = AccountsBackgroundService::new(
            bank_forks.clone(),
            exit,
            AbsRequestHandlers {
                snapshot_request_handler,
                pruned_banks_request_handler,
            },
            config.accounts_db_test_hash_calculation,
            last_full_snapshot_slot,
        );
        Ok(Self {
            accounts_background_service,
            accounts_hash_verifier,
            accounts_background_request_sender,
            snapshot_packager_service,
        })
    }

    pub fn join(self) {
        self.accounts_background_service
            .join()
            .expect("accounts_background_service");
        self.accounts_hash_verifier
            .join()
            .expect("accounts_hash_verifier");
        self.snapshot_packager_service
            .join()
            .expect("snapshot_packager_service");
    }
}
