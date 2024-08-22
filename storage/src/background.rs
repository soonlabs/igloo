use std::sync::{atomic::AtomicBool, Arc, RwLock};

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
    snapshot_hash::StartingSnapshotHashes,
};

use crate::{config::StorageConfig, sig_hub::SignalHub, Error, Result};

#[allow(dead_code)]
pub struct StorageBackground {
    pub(crate) accounts_background_service: AccountsBackgroundService,
    pub(crate) accounts_background_request_sender: AbsRequestSender,
    pub(crate) accounts_hash_verifier: AccountsHashVerifier,
    pub(crate) snapshot_packager_service: SnapshotPackagerService,
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
            pruned_banks_receiver: hub
                .pruned_banks_receiver
                .take()
                .ok_or(Error::InitCommon(format!("pruned banks receiver is None")))?,
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
