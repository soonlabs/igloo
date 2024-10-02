use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_STAKE_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::{assert_result_balance, processor::process_transfers_ex},
    RollupStorage,
};
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
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
    assert!(accounts_status.num_accounts > 200);
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
    assert!(accounts_status.num_accounts > 200);

    Ok(())
}

#[tokio::test]
async fn storage_basic_process_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let config = GlobalConfig::new_temp(&ledger_path)?;
    let mut store = RollupStorage::new(config)?;
    store.init()?;
    let keypairs = store.config.keypairs.clone();

    let mut reload_config = GlobalConfig::new(&ledger_path)?;
    reload_config.keypairs = keypairs.clone();

    basic_process_tests(
        store,
        reload_config,
        keypairs.mint_keypair.as_ref().unwrap().as_ref(),
        keypairs.validator_keypair.as_ref().unwrap().as_ref(),
        Keypair::new().pubkey(),
        Keypair::new().pubkey(),
        DEFAULT_MINT_LAMPORTS,
        DEFAULT_VALIDATOR_LAMPORTS,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn basic_process_tests(
    mut store: RollupStorage,
    reload_config: GlobalConfig,
    alice: &Keypair,
    bob: &Keypair,
    charlie: Pubkey,
    dave: Pubkey,
    alice_init_balance: u64,
    bob_init_balance: u64,
) -> Result<()> {
    assert_eq!(store.balance(&alice.pubkey()), alice_init_balance);
    assert_eq!(store.balance(&bob.pubkey()), bob_init_balance);
    assert_eq!(store.balance(&charlie), 0);
    assert_eq!(store.balance(&dave), 0);

    const TO_CHARLIE: u64 = 2000000;
    const TO_DAVE: u64 = 1000000;
    const FEE: u64 = 5000;

    // 1. process transfers
    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        system_transaction::transfer(alice, &charlie, TO_CHARLIE, bank.last_blockhash()),
        system_transaction::transfer(bob, &dave, TO_DAVE, bank.last_blockhash()),
    ];
    let origin_txs = raw_txs
        .clone()
        .into_iter()
        .map(SanitizedTransaction::from_transaction_for_tests)
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
        results.loaded_transactions[0].as_ref().unwrap(),
    );
    assert_result_balance(
        alice.pubkey(),
        Some(alice_init_balance - TO_CHARLIE - FEE),
        results.loaded_transactions[0].as_ref().unwrap(),
    );
    // assert second transaction result
    assert_result_balance(
        alice.pubkey(),
        None, // alice balance not changed
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        bob.pubkey(),
        Some(bob_init_balance - TO_DAVE - FEE), // TODO: should be INIT_AMOUNT + BOB_PLUS - BOB_MINUS ?
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        dave,
        Some(TO_DAVE),
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    // after process balance not changed
    assert_eq!(store.balance(&alice.pubkey()), alice_init_balance);
    assert_eq!(store.balance(&bob.pubkey()), bob_init_balance);
    assert_eq!(store.balance(&charlie), 0);
    assert_eq!(store.balance(&dave), 0);

    // 2. commit
    store
        .commit(
            vec![TransactionsResultWrapper { output: results }],
            vec![CommitBatch::new(origin_txs.clone().into())],
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    // after commit balance changed
    assert_eq!(
        store.balance(&alice.pubkey()),
        alice_init_balance - TO_CHARLIE - FEE
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        bob_init_balance - TO_DAVE - FEE
    );
    assert_eq!(store.balance(&charlie), TO_CHARLIE);
    assert_eq!(store.balance(&dave), TO_DAVE);

    // 3. save and close
    store.confirm(store.current_height())?;
    store.close().await?;

    // 4. open again
    let mut store = RollupStorage::new(reload_config)?;
    store.init()?;
    let ticks_per_slot = store.config.genesis.ticks_per_slot;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    // TODO: check why bob balance is not `alice_init_balance - TO_CHARLIE` ?
    assert_eq!(
        store.balance(&alice.pubkey()),
        alice_init_balance - TO_CHARLIE
    );
    // TODO: check why bob balance is not `bob_init_balance - TO_DAVE - FEE` ?
    assert_eq!(store.balance(&bob.pubkey()), bob_init_balance - TO_DAVE);
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
