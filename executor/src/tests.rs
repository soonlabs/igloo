use anyhow::Result;
use igloo_storage::{
    config::GlobalConfig,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
};
use solana_sdk::{
    signature::Keypair, signer::Signer, system_transaction, transaction::SanitizedTransaction,
};

use crate::{BlockPayload, Error, Executor};

#[tokio::test]
async fn engine_basic_process_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut engine = Executor::new_for_test(&ledger_path)?;
    let keypairs = engine.storage()?.keypairs().clone();

    let alice = keypairs.mint_keypair.as_ref().unwrap().clone();
    let bob = keypairs.validator_keypair.as_ref().unwrap().clone();
    let charlie = Keypair::new().pubkey();
    let dave = Keypair::new().pubkey();

    const ALICE_INIT_BALANCE: u64 = DEFAULT_MINT_LAMPORTS;
    const BOB_INIT_BALANCE: u64 = DEFAULT_VALIDATOR_LAMPORTS;
    assert_eq!(
        engine.storage()?.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE
    );
    assert_eq!(engine.storage()?.balance(&bob.pubkey()), BOB_INIT_BALANCE);
    assert_eq!(engine.storage()?.balance(&charlie), 0);
    assert_eq!(engine.storage()?.balance(&dave), 0);

    const TO_CHARLIE: u64 = 2000000;
    const TO_DAVE: u64 = 1000000;

    let raw_txs = vec![
        system_transaction::transfer(
            &alice,
            &charlie,
            TO_CHARLIE,
            engine.storage()?.current_bank().last_blockhash(),
        ),
        system_transaction::transfer(
            &bob,
            &dave,
            TO_DAVE,
            engine.storage()?.current_bank().last_blockhash(),
        ),
    ];
    let block_payload = BlockPayload {
        transactions: vec![raw_txs
            .clone()
            .into_iter()
            .map(SanitizedTransaction::from_transaction_for_tests)
            .collect::<Vec<_>>()],
    };

    // we can check block before processing
    assert!(engine.check_block(&block_payload, None).is_ok());

    let info = engine.new_block(block_payload).await?;
    assert_eq!(info.head.slot, 1);
    assert_eq!(info.store_height, Some(1));
    assert_eq!(info.parent.slot, 0);

    // after commit new block balance changed
    assert_eq!(
        engine.storage()?.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE
    );
    assert_eq!(
        engine.storage()?.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - TO_DAVE
    );
    assert_eq!(engine.storage()?.balance(&charlie), TO_CHARLIE);
    assert_eq!(engine.storage()?.balance(&dave), TO_DAVE);

    // save and close
    engine.close().await?;

    // open again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    let engine = Executor::new_with_config(config)?;

    let (bank_height, store_height) = engine.storage()?.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    assert_eq!(
        engine.storage()?.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE
    );
    assert_eq!(
        engine.storage()?.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - TO_DAVE
    );
    assert_eq!(engine.storage()?.balance(&charlie), TO_CHARLIE);
    assert_eq!(engine.storage()?.balance(&dave), TO_DAVE);
    engine.close().await?;

    Ok(())
}

#[tokio::test]
async fn commit_empty_block_should_fail() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut engine = Executor::new_for_test(&ledger_path)?;

    let block_payload = BlockPayload {
        transactions: vec![],
    };

    // we can check block before processing
    assert!(engine.check_block(&block_payload, None).is_ok());

    let result = engine.new_block(block_payload).await;
    assert!(matches!(
        result,
        Err(Error::StorageError(igloo_storage::Error::NoEntries))
    ));

    Ok(())
}
