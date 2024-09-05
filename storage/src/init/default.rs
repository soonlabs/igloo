use crate::{config::MAX_GENESIS_ARCHIVE_UNPACKED_SIZE, Result};
use solana_ledger::{
    blockstore::create_new_ledger, blockstore_options::LedgerColumnOptions,
    genesis_utils::GenesisConfigInfo,
};
use solana_runtime::genesis_utils::create_genesis_config_with_leader_ex;
use solana_sdk::{
    fee_calculator::FeeRateGovernor,
    genesis_config::{ClusterType, GenesisConfig},
    rent::Rent,
    signature::Keypair,
    signer::Signer,
};
use std::path::Path;

pub const DEFAULT_VALIDATOR_LAMPORTS: u64 = 10_000_000;
pub const DEFAULT_MINT_LAMPORTS: u64 = 1_000_000_000;
pub const DEFAULT_STAKE_LAMPORTS: u64 = 50_000_000;

pub(crate) fn default_genesis_config(ledger_path: &Path) -> Result<(GenesisConfigInfo, Keypair)> {
    let validator_key = Keypair::new();
    let mint_keypair = Keypair::new();
    let voting_keypair = Keypair::new();
    let genesis_config = create_genesis_config_with_leader_ex(
        DEFAULT_MINT_LAMPORTS,
        &mint_keypair.pubkey(),
        &validator_key.pubkey(),
        &voting_keypair.pubkey(),
        &solana_sdk::pubkey::new_rand(),
        DEFAULT_STAKE_LAMPORTS,
        DEFAULT_VALIDATOR_LAMPORTS,
        FeeRateGovernor::new(0, 0), // most tests can't handle transaction fees
        Rent::free(),               // most tests don't expect rent
        ClusterType::Development,
        vec![],
    );
    init_block_store(ledger_path, &genesis_config)?;

    Ok((
        GenesisConfigInfo {
            genesis_config,
            mint_keypair,
            voting_keypair,
            validator_pubkey: validator_key.pubkey(),
        },
        validator_key,
    ))
}

fn init_block_store(ledger_path: &Path, genesis_config: &GenesisConfig) -> Result<()> {
    let hash = create_new_ledger(
        ledger_path,
        genesis_config,
        MAX_GENESIS_ARCHIVE_UNPACKED_SIZE,
        LedgerColumnOptions::default(),
    )?;
    info!("Create new ledger done, new genesis hash: {}", hash);

    Ok(())
}
