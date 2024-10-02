use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::{
        assert_result_balance,
        processor::process_transfers,
        system_account,
        txs::{create_svm_transactions, MockTransaction},
    },
    RollupStorage,
};
use anyhow::Result;
use solana_sdk::{signature::Keypair, signer::Signer, system_transaction};

#[tokio::test]
async fn conflict_transaction_may_lead_incorrect_state() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let config = GlobalConfig::new_dev(&ledger_path)?;
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let alice = Keypair::new().pubkey();
    let bob = Keypair::new().pubkey();
    let charlie = Keypair::new().pubkey();
    let payer = Keypair::new().pubkey();

    assert_eq!(store.balance(&alice), 0);
    assert_eq!(store.balance(&bob), 0);
    assert_eq!(store.balance(&charlie), 0);

    // 1. init accounts
    const INIT_AMOUNT: u64 = 10_000_000;
    store.bump()?;
    assert_eq!(store.current_height(), 1);
    [
        (alice, system_account(INIT_AMOUNT)),
        (bob, system_account(INIT_AMOUNT)),
        (charlie, system_account(INIT_AMOUNT)),
        (payer, system_account(INIT_AMOUNT)),
    ]
    .into_iter()
    .for_each(|(key, data)| store.insert_account(key, data).unwrap());

    assert_eq!(store.balance(&alice), INIT_AMOUNT);
    assert_eq!(store.balance(&bob), INIT_AMOUNT);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT);

    // 2. process transfers
    const BOB_PLUS: u64 = 2_000;
    const BOB_MINUS: u64 = 5_000;
    let raw_txs = vec![
        MockTransaction {
            from: alice,
            to: bob,
            amount: BOB_PLUS,
            payer: Some(payer),
        },
        MockTransaction {
            from: bob,
            to: charlie,
            amount: BOB_MINUS,
            payer: Some(payer),
        },
    ];
    let results = process_transfers(&store, &raw_txs);

    assert_eq!(results.execution_results.len(), 2);
    for result in results.execution_results.iter() {
        assert!(result.was_executed());
        assert!(result.details().unwrap().status.is_ok());
    }
    // assert first transaction result
    assert_result_balance(
        bob,
        Some(INIT_AMOUNT + BOB_PLUS),
        results.loaded_transactions[0].as_ref().unwrap(),
    );
    assert_result_balance(
        alice,
        Some(INIT_AMOUNT - BOB_PLUS),
        results.loaded_transactions[0].as_ref().unwrap(),
    );
    // assert second transaction result
    assert_result_balance(
        alice,
        None, // alice balance not changed
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        bob,
        Some(INIT_AMOUNT - BOB_MINUS), // TODO: should be INIT_AMOUNT + BOB_PLUS - BOB_MINUS or throw error ?
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        charlie,
        Some(INIT_AMOUNT + BOB_MINUS),
        results.loaded_transactions[1].as_ref().unwrap(),
    );
    // after process balance not changed
    assert_eq!(store.balance(&alice), INIT_AMOUNT);
    assert_eq!(store.balance(&bob), INIT_AMOUNT);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT);

    // 3. commit
    let origin_txs = create_svm_transactions(&raw_txs);
    store
        .commit(
            vec![TransactionsResultWrapper { output: results }],
            vec![CommitBatch::new(origin_txs.clone().into())],
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));

    assert_eq!(store.balance(&alice), INIT_AMOUNT - BOB_PLUS);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT + BOB_MINUS);
    // TODO: bob balance should be INIT_AMOUNT + BOB_PLUS - BOB_MINUS
    assert_eq!(store.balance(&bob), INIT_AMOUNT - BOB_MINUS);

    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn conflict_transaction_execute_with_bank() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;
    config
        .storage
        .snapshot_config
        .full_snapshot_archive_interval_slots = 1; // set snapshot interval to 1
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let keypairs = store.config.keypairs.clone();

    let alice = keypairs.mint_keypair.as_ref().unwrap();
    let bob = Keypair::new();
    let charlie = Keypair::new();

    const ALICE_INIT_BALANCE: u64 = DEFAULT_MINT_LAMPORTS;
    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE);
    assert_eq!(store.balance(&bob.pubkey()), 0);
    assert_eq!(store.balance(&charlie.pubkey()), 0);

    const TO_BOB: u64 = 1000000;
    const TO_CHARLIE: u64 = 2000000;

    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        system_transaction::transfer(alice, &bob.pubkey(), TO_BOB, bank.last_blockhash()),
        system_transaction::transfer(alice, &charlie.pubkey(), TO_CHARLIE, bank.last_blockhash()),
    ];
    let results = bank.process_transactions(raw_txs.iter());
    assert_eq!(results[0], Ok(()));
    assert!(results[1].is_err()); // Account in use error

    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE - TO_BOB);
    assert_eq!(store.balance(&bob.pubkey()), TO_BOB);
    assert_eq!(store.balance(&charlie.pubkey()), 0);

    store.close().await?;

    Ok(())
}

#[tokio::test]
async fn conflict_transaction_execute_with_bank2() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;
    config
        .storage
        .snapshot_config
        .full_snapshot_archive_interval_slots = 1; // set snapshot interval to 1
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let keypairs = store.config.keypairs.clone();

    let alice = keypairs.mint_keypair.as_ref().unwrap();
    let bob = keypairs.validator_keypair.as_ref().unwrap();
    let charlie = Keypair::new();

    const ALICE_INIT_BALANCE: u64 = DEFAULT_MINT_LAMPORTS;
    const BOB_INIT_BALANCE: u64 = DEFAULT_VALIDATOR_LAMPORTS;
    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE);
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE);
    assert_eq!(store.balance(&charlie.pubkey()), 0);

    const TO_BOB: u64 = 1000000;
    const TO_CHARLIE: u64 = 2000000;

    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        system_transaction::transfer(alice, &bob.pubkey(), TO_BOB, bank.last_blockhash()),
        system_transaction::transfer(bob, &charlie.pubkey(), TO_CHARLIE, bank.last_blockhash()),
    ];
    let results = bank.process_transactions(raw_txs.iter());
    assert_eq!(results[0], Ok(()));
    assert!(results[1].is_err()); // Account in use error

    assert_eq!(store.balance(&alice.pubkey()), ALICE_INIT_BALANCE - TO_BOB);
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE + TO_BOB);
    assert_eq!(store.balance(&charlie.pubkey()), 0);

    store.close().await?;
    Ok(())
}
