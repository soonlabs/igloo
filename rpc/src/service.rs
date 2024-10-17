use crate::jsonrpc::core::JsonRpcConfig;
use crate::jsonrpc::service::JsonRpcService;
use crate::Result;
use crossbeam_channel::{Receiver, Sender};
use igloo_storage::RollupStorage;
use solana_ledger::blockstore::Blockstore;
use solana_runtime::bank_forks::BankForks;
use solana_runtime::prioritization_fee_cache::PrioritizationFeeCache;
use solana_runtime::snapshot_config::SnapshotConfig;
use solana_sdk::exit::Exit;
use solana_sdk::hash::Hash;
use solana_sdk::transaction::SanitizedTransaction;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub rpc_addr: SocketAddr,
    pub jsonrpc_config: JsonRpcConfig,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            rpc_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8899)),
            jsonrpc_config: JsonRpcConfig::default(),
        }
    }
}

pub struct RpcService {
    // Note: In current implementation, we removed `solana_send_transaction_service`, which sent
    // transactions to TPU. We do not consider to make the rpc service independently right now, the
    // exposed transaction receiver below should be used by transaction stream module directly.
    pub jsonrpc: JsonRpcService,
}

impl RpcService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rpc_config: RpcConfig,
        tx_channel: (Sender<SanitizedTransaction>, Receiver<SanitizedTransaction>),
        node_exit: Arc<RwLock<Exit>>,
        storage: &RollupStorage,
    ) -> Result<Self> {
        let storage_config = storage.config();
        let jsonrpc = Self::new_rpc_service(
            &rpc_config,
            storage_config.storage.snapshot_config.clone(),
            storage.bank_forks(),
            storage.blockstore(),
            storage_config
                .storage
                .expected_genesis_hash
                .unwrap_or(storage_config.genesis.hash()),
            tx_channel,
            storage_config.ledger_path.as_path(),
            node_exit.clone(),
            storage
                .history_services()
                .max_complete_transaction_status_slot
                .clone(),
        )?;

        if storage_config.storage.halt_at_slot.is_some() {
            // Park with the RPC service running, ready for inspection!
            warn!("Validator halted");
            std::thread::park();
        }

        Ok(Self { jsonrpc })
    }

    pub fn join(self) {
        self.jsonrpc.join().expect("jsonrpc_service");
    }

    #[allow(clippy::too_many_arguments)]
    fn new_rpc_service(
        rpc_config: &RpcConfig,
        snapshot_config: SnapshotConfig,
        bank_forks: Arc<RwLock<BankForks>>,
        blockstore: Arc<Blockstore>,
        genesis_hash: Hash,
        tx_channel: (Sender<SanitizedTransaction>, Receiver<SanitizedTransaction>),
        ledger_path: &Path,
        node_exit: Arc<RwLock<Exit>>,
        max_complete_transaction_status_slot: Arc<AtomicU64>,
    ) -> Result<JsonRpcService> {
        // block min prioritization fee cache should be readable by RPC, and writable by validator
        // (by both replay stage and banking stage)
        let prioritization_fee_cache = Arc::new(PrioritizationFeeCache::default());

        JsonRpcService::new(
            rpc_config.rpc_addr,
            rpc_config.jsonrpc_config.clone(),
            Some(snapshot_config),
            bank_forks,
            blockstore,
            genesis_hash,
            tx_channel,
            ledger_path,
            node_exit,
            max_complete_transaction_status_slot.clone(),
            prioritization_fee_cache,
        )
        .map_err(crate::Error::InitJsonRpc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use igloo_storage::config::GlobalConfig;
    use solana_client::rpc_client::RpcClient;

    fn new_rpc_service(storage: &mut RollupStorage, rpc_config: RpcConfig) -> Result<RpcService> {
        let node_exit = Arc::new(RwLock::new(Exit::default()));

        let rpc_service = RpcService::new(
            rpc_config,
            crossbeam_channel::unbounded(),
            node_exit,
            storage,
        )?;

        Ok(rpc_service)
    }

    #[test]
    fn test_json_rpc_service() -> Result<()> {
        let ledger_path = tempfile::tempdir()?.into_path();
        let config = GlobalConfig::new_dev(&ledger_path)?;
        let mut storage = RollupStorage::new(config)?;
        storage.init()?;

        let rpc_config = RpcConfig::default();
        let rpc_service = new_rpc_service(&mut storage, rpc_config.clone())?;

        // Test the JSON RPC service
        let client = RpcClient::new_socket(rpc_config.rpc_addr);
        let hash = client
            .get_genesis_hash()
            .expect("Failed to get genesis hash");
        assert_eq!(hash, storage.config().genesis.hash());

        rpc_service.join();
        Ok(())
    }
}
