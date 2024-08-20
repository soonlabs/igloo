use std::sync::{atomic::AtomicBool, Arc, RwLock};

use crossbeam_channel::unbounded;
use solana_core::accounts_hash_verifier::AccountsHashVerifier;
use solana_runtime::{
    accounts_background_service::{
        AbsRequestHandlers, AbsRequestSender, AccountsBackgroundService, PrunedBanksRequestHandler,
        SnapshotRequestHandler,
    },
    bank_forks::BankForks,
};

use crate::{config::StorageConfig, sig_hub::SignalHub, Error, Result};

// TODO: implement storage background
#[allow(dead_code)]
pub struct StorageBackground {
    pub(crate) accounts_background_service: AccountsBackgroundService,
    pub(crate) accounts_background_request_sender: AbsRequestSender,
    pub(crate) accounts_hash_verifier: AccountsHashVerifier,
}

impl StorageBackground {
    pub fn new(
        bank_forks: Arc<RwLock<BankForks>>,
        config: &StorageConfig,
        hub: &mut SignalHub,
        exit: Arc<AtomicBool>,
    ) -> Result<Self> {
        let (accounts_package_sender, accounts_package_receiver) = crossbeam_channel::unbounded();
        let accounts_hash_verifier = AccountsHashVerifier::new(
            accounts_package_sender.clone(),
            accounts_package_receiver,
            None,
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

        let accounts_background_service = AccountsBackgroundService::new(
            bank_forks.clone(),
            exit,
            AbsRequestHandlers {
                snapshot_request_handler,
                pruned_banks_request_handler,
            },
            config.accounts_db_test_hash_calculation,
            None,
        );
        Ok(Self {
            accounts_background_service,
            accounts_hash_verifier,
            accounts_background_request_sender,
        })
    }
}
