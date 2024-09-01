use anyhow::Result;
use rollups_storage::{
    config::GlobalConfig,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
};
use solana_sdk::{
    signature::Keypair, signer::Signer, system_transaction, transaction::SanitizedTransaction,
};

use crate::{defs::BlockPayload, Engine};

#[tokio::test]
async fn engine_basic_process_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut engine = Engine::new_for_test(&ledger_path)?;
    let keypairs = engine.store().keypairs().clone();

    let alice = keypairs.mint_keypair.as_ref().unwrap().clone();
    let bob = keypairs.validator_keypair.as_ref().unwrap().clone();
    let charlie = Keypair::new().pubkey();
    let dave = Keypair::new().pubkey();

    const ALICE_INIT_BALANCE: u64 = DEFAULT_MINT_LAMPORTS;
    const BOB_INIT_BALANCE: u64 = DEFAULT_VALIDATOR_LAMPORTS;
    assert_eq!(engine.store().balance(&alice.pubkey()), ALICE_INIT_BALANCE);
    assert_eq!(engine.store().balance(&bob.pubkey()), BOB_INIT_BALANCE);
    assert_eq!(engine.store().balance(&charlie), 0);
    assert_eq!(engine.store().balance(&dave), 0);

    const TO_CHARLIE: u64 = 2000000;
    const TO_DAVE: u64 = 1000000;

    let raw_txs = vec![
        system_transaction::transfer(
            &alice,
            &charlie,
            TO_CHARLIE,
            engine.store().current_bank().last_blockhash(),
        ),
        system_transaction::transfer(
            &bob,
            &dave,
            TO_DAVE,
            engine.store().current_bank().last_blockhash(),
        ),
    ];
    let block_payload = BlockPayload {
        transactions: raw_txs
            .clone()
            .into_iter()
            .map(|tx| SanitizedTransaction::from_transaction_for_tests(tx))
            .collect::<Vec<_>>(),
    };

    // we can check block before processing
    assert!(engine.check_block(&block_payload, None).is_ok());

    let info = engine.new_block(block_payload).await?;
    assert_eq!(info.head.slot, 1);
    assert_eq!(info.store_height, Some(1));
    assert_eq!(info.parent.slot, 0);

    // after commit new block balance changed
    assert_eq!(
        engine.store().balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE
    );
    assert_eq!(
        engine.store().balance(&bob.pubkey()),
        BOB_INIT_BALANCE - TO_DAVE
    );
    assert_eq!(engine.store().balance(&charlie), TO_CHARLIE);
    assert_eq!(engine.store().balance(&dave), TO_DAVE);

    // save and close
    engine.confirm(info.head.slot)?;
    engine.close().await?;

    // open again
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    let engine = Engine::new_with_config(config)?;

    let (bank_height, store_height) = engine.store().get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    assert_eq!(
        engine.store().balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE
    );
    assert_eq!(
        engine.store().balance(&bob.pubkey()),
        BOB_INIT_BALANCE - TO_DAVE
    );
    assert_eq!(engine.store().balance(&charlie), TO_CHARLIE);
    assert_eq!(engine.store().balance(&dave), TO_DAVE);
    engine.close().await?;

    Ok(())
}
