use std::path::Path;

use crate::{
    blockstore::txs::CommitBatch,
    config::{GlobalConfig, KeypairsConfig},
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_STAKE_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::{assert_result_balance, processor::process_transfers_ex},
    RollupStorage,
};
use anyhow::Result;
use rollups_interface::l2::{bank::BankOperations, storage::StorageOperations};
use solana_sdk::{
    signature::Keypair,
    signer::Signer,
    system_transaction,
    transaction::{SanitizedTransaction, VersionedTransaction},
};

#[tokio::test]
async fn init_with_all_default_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut store = RollupStorage::new(GlobalConfig::new_temp(&ledger_path)?)?;
    store.init()?;
    let keypairs = store.config.keypairs.clone();

    assert!(store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    assert_eq!(
        store.balance(&keypairs.validator_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_VALIDATOR_LAMPORTS,
    );
    assert_eq!(
        store.balance(&keypairs.mint_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_MINT_LAMPORTS,
    );
    assert_eq!(
        store.balance(&keypairs.voting_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_STAKE_LAMPORTS,
    );
    let accounts_status = store.all_accounts_status()?;
    assert_eq!(accounts_status.num_accounts, 222);
    store.close().await?;

    // create store again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs.clone();
    let mut store = RollupStorage::new(config)?;
    store.init()?;
    assert!(!store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    assert_eq!(
        store.balance(&keypairs.validator_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_VALIDATOR_LAMPORTS,
    );
    assert_eq!(
        store.balance(&keypairs.mint_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_MINT_LAMPORTS,
    );
    assert_eq!(
        store.balance(&keypairs.voting_keypair.as_ref().unwrap().pubkey()),
        DEFAULT_STAKE_LAMPORTS,
    );
    let accounts_status = store.all_accounts_status()?;
    assert_eq!(accounts_status.num_accounts, 222);

    Ok(())
}

#[test]
fn init_with_given_config_works() -> Result<()> {
    let mut config = GlobalConfig::new(&Path::new("data/config/ledger"))?;
    config.keypairs = KeypairsConfig {
        validator_key_path: Some("data/config/genesis/validator-identity.json".into()),
        mint_key_path: Some("data/config/genesis/validator-stake-account.json".into()),
        voting_key_path: Some("data/config/genesis/validator-vote-account.json".into()),
        ..Default::default()
    };
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let keypairs = store.config.keypairs.clone();
    assert!(!store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    assert_eq!(
        store.balance(&keypairs.validator_keypair.as_ref().unwrap().pubkey()),
        500000000000
    );
    assert_eq!(
        store.balance(&keypairs.mint_keypair.as_ref().unwrap().pubkey()),
        500000000,
    );
    assert_eq!(
        store.balance(&keypairs.voting_keypair.as_ref().unwrap().pubkey()),
        27074400
    );
    Ok(())
}

#[tokio::test]
async fn storage_basic_process_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let config = GlobalConfig::new_temp(&ledger_path)?;
    let mut store = RollupStorage::new(config)?;
    store.init()?;
    let keypairs = store.config.keypairs.clone();

    store.set_snapshot_interval(1);
    assert_eq!(store.current_height(), 0);

    let alice = keypairs.mint_keypair.as_ref().unwrap().clone();
    let bob = keypairs.validator_keypair.as_ref().unwrap().clone();
    let charlie = Keypair::new().pubkey();
    let dave = Keypair::new().pubkey();

    const ALICE_INIT_BALANCE: u64 = DEFAULT_MINT_LAMPORTS;
    const BOB_INIT_BALANCE: u64 = DEFAULT_VALIDATOR_LAMPORTS;
    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE);
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE);
    assert_eq!(store.balance(&charlie), 0);
    assert_eq!(store.balance(&dave), 0);

    const TO_CHARLIE: u64 = 2000000;
    const TO_DAVE: u64 = 1000000;
    const FEE: u64 = 5000;

    // 1. process transfers
    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        system_transaction::transfer(&alice, &charlie, TO_CHARLIE, bank.last_blockhash()),
        system_transaction::transfer(&bob, &dave, TO_DAVE, bank.last_blockhash()),
    ];
    let origin_txs = raw_txs
        .clone()
        .into_iter()
        .map(|tx| SanitizedTransaction::from_transaction_for_tests(tx))
        .collect::<Vec<_>>();
    let results = process_transfers_ex(&store, origin_txs.clone());

    assert_eq!(results.execution_results.len(), 2);
    for result in results.execution_results.iter() {
        assert!(result.was_executed());
        assert!(result.details().unwrap().status.is_ok());
    }
    // assert first transaction result
    assert_result_balance(
        charlie,
        Some(TO_CHARLIE),
        &results.loaded_transactions[0].as_ref().unwrap(),
    );
    assert_result_balance(
        alice.pubkey(),
        Some(ALICE_INIT_BALANCE - TO_CHARLIE - FEE),
        &results.loaded_transactions[0].as_ref().unwrap(),
    );
    // assert second transaction result
    assert_result_balance(
        alice.pubkey(),
        None, // alice balance not changed
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        bob.pubkey(),
        Some(BOB_INIT_BALANCE - TO_DAVE - FEE), // TODO: should be INIT_AMOUNT + BOB_PLUS - BOB_MINUS ?
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        dave,
        Some(TO_DAVE),
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    // after process balance not changed
    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE);
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE);
    assert_eq!(store.balance(&charlie), 0);
    assert_eq!(store.balance(&dave), 0);

    // 2. commit
    store
        .commit(
            TransactionsResultWrapper { output: results },
            &CommitBatch::new(origin_txs.clone().into()),
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    // after commit balance changed
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE - FEE
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - TO_DAVE - FEE
    );
    assert_eq!(store.balance(&charlie), TO_CHARLIE);
    assert_eq!(store.balance(&dave), TO_DAVE);

    // 3. save and close
    store.force_save().await?;
    // TODO: sleep is needed here, improve later
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    store.close().await?;

    // 4. open again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    let ticks_per_slot = config.genesis.ticks_per_slot;
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    // TODO: check why bob balance is not `ALICE_INIT_BALANCE - TO_CHARLIE` ?
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE
    );
    // TODO: check why bob balance is not `BOB_INIT_BALANCE - TO_DAVE - FEE` ?
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE - TO_DAVE);
    assert_eq!(store.balance(&charlie), TO_CHARLIE);
    assert_eq!(store.balance(&dave), TO_DAVE);

    // load entries from blockstore, compare with original transactions
    let entries = store.blockstore.get_slot_entries(1, 0)?;
    assert_eq!(entries.len() as u64, ticks_per_slot + 1);
    assert_eq!(entries[0].transactions.len(), 2);
    assert_eq!(
        entries[0].transactions[0],
        VersionedTransaction::from(raw_txs[0].clone())
    );
    assert_eq!(
        entries[0].transactions[1],
        VersionedTransaction::from(raw_txs[1].clone())
    );
    store.close().await?;

    Ok(())
}
