use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::processor::process_transfers_ex,
    RollupStorage,
};
use anyhow::Result;
use solana_sdk::{
    signature::Keypair, signer::Signer, system_transaction, transaction::SanitizedTransaction,
};

#[tokio::test]
async fn history_storage_works() -> Result<()> {
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
    #[allow(dead_code)]
    const FEE: u64 = 5000;

    // process transfers
    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        system_transaction::transfer(&alice, &charlie, TO_CHARLIE, bank.last_blockhash()),
        system_transaction::transfer(&bob, &dave, TO_DAVE, bank.last_blockhash()),
    ];
    let origin_txs = raw_txs
        .clone()
        .into_iter()
        .map(SanitizedTransaction::from_transaction_for_tests)
        .collect::<Vec<_>>();
    let results = process_transfers_ex(&store, origin_txs.clone());

    // commit
    store
        .commit(
            vec![TransactionsResultWrapper { output: results }],
            vec![CommitBatch::new(origin_txs.clone().into())],
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));

    // save and close
    store.force_save().await?;
    store.close().await?;

    // open again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    // query transaction meta of the first transaction
    let tx1_with_meta = store
        .get_transaction_meta(*origin_txs[0].signature(), None)?
        .unwrap();
    assert_eq!(tx1_with_meta.slot, 1);
    let tx1_status = tx1_with_meta.tx_with_meta.get_status_meta().unwrap();
    assert_eq!(tx1_status.status, Ok(()));
    // assert_eq!(tx1_status.fee, FEE); // FIXME: fee is not calculated correctly
    assert_eq!(tx1_status.pre_balances, vec![ALICE_INIT_BALANCE, 0, 1]);
    assert_eq!(
        tx1_status.post_balances,
        // FIXME: first item should be ALICE_INIT_BALANCE - TO_CHARLIE - FEE
        vec![ALICE_INIT_BALANCE - TO_CHARLIE, TO_CHARLIE, 1]
    );

    // query transaction meta of the second transaction
    let tx2_with_meta = store
        .get_transaction_meta(*origin_txs[1].signature(), None)?
        .unwrap();
    assert_eq!(tx2_with_meta.slot, 1);
    let tx2_status = tx2_with_meta.tx_with_meta.get_status_meta().unwrap();
    assert_eq!(tx2_status.status, Ok(()));
    // assert_eq!(tx2_status.fee, FEE); // FIXME: fee is not calculated correctly
    assert_eq!(tx2_status.pre_balances, vec![BOB_INIT_BALANCE, 0, 1]);
    assert_eq!(
        tx2_status.post_balances,
        // FIXME: first item should be BOB_INIT_BALANCE - TO_DAVE - FEE
        vec![BOB_INIT_BALANCE - TO_DAVE, TO_DAVE, 1]
    );

    // query block related meta
    assert!(tx1_with_meta.block_time.is_some());
    assert_eq!(tx1_with_meta.block_time, tx2_with_meta.block_time);
    assert_eq!(store.blockstore.get_block_height(1).unwrap(), Some(1));

    store.close().await?;

    Ok(())
}

#[tokio::test]
async fn history_storage_with_multi_entries_works() -> Result<()> {
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
    #[allow(dead_code)]
    const FEE: u64 = 5000;

    // process transfers
    store.bump()?;
    let bank = store.bank.clone();

    let raw_txs = vec![
        vec![system_transaction::transfer(
            &alice,
            &charlie,
            TO_CHARLIE,
            bank.last_blockhash(),
        )],
        vec![system_transaction::transfer(
            &bob,
            &dave,
            TO_DAVE,
            bank.last_blockhash(),
        )],
    ];

    let mut results = vec![];
    let mut origins = vec![];
    for txs in raw_txs {
        let txs = txs
            .into_iter()
            .map(SanitizedTransaction::from_transaction_for_tests)
            .collect::<Vec<_>>();
        results.push(TransactionsResultWrapper {
            output: process_transfers_ex(&store, txs.clone()),
        });
        origins.push(txs);
    }

    // commit
    store
        .commit(
            results,
            origins
                .iter()
                .map(|txs| CommitBatch::new(txs.clone().into()))
                .collect(),
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));

    // save and close
    store.force_save().await?;
    store.close().await?;

    // open again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    // query transaction meta of the first transaction
    let tx1_with_meta = store
        .get_transaction_meta(*origins[0][0].signature(), None)?
        .unwrap();
    assert_eq!(tx1_with_meta.slot, 1);
    let tx1_status = tx1_with_meta.tx_with_meta.get_status_meta().unwrap();
    assert_eq!(tx1_status.status, Ok(()));
    // assert_eq!(tx1_status.fee, FEE); // FIXME: fee is not calculated correctly
    assert_eq!(tx1_status.pre_balances, vec![ALICE_INIT_BALANCE, 0, 1]);
    assert_eq!(
        tx1_status.post_balances,
        // FIXME: first item should be ALICE_INIT_BALANCE - TO_CHARLIE - FEE
        vec![ALICE_INIT_BALANCE - TO_CHARLIE, TO_CHARLIE, 1]
    );

    // query transaction meta of the second transaction
    let tx2_with_meta = store
        .get_transaction_meta(*origins[1][0].signature(), None)?
        .unwrap();
    assert_eq!(tx2_with_meta.slot, 1);
    let tx2_status = tx2_with_meta.tx_with_meta.get_status_meta().unwrap();
    assert_eq!(tx2_status.status, Ok(()));
    // assert_eq!(tx2_status.fee, FEE); // FIXME: fee is not calculated correctly
    assert_eq!(tx2_status.pre_balances, vec![BOB_INIT_BALANCE, 0, 1]);
    assert_eq!(
        tx2_status.post_balances,
        // FIXME: first item should be BOB_INIT_BALANCE - TO_DAVE - FEE
        vec![BOB_INIT_BALANCE - TO_DAVE, TO_DAVE, 1]
    );

    // query block related meta
    assert!(tx1_with_meta.block_time.is_some());
    assert_eq!(tx1_with_meta.block_time, tx2_with_meta.block_time);
    assert_eq!(store.blockstore.get_block_height(1).unwrap(), Some(1));

    store.close().await?;

    Ok(())
}

// TODO: add unit test with commit success and failed transactions, this can be done after
//       transaction execution check is implemented
