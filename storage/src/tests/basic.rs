use std::path::Path;

use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    tests::mock::{
        processor::process_transfers,
        txs::{create_svm_transactions, MockTransaction},
    },
    RollupStorage,
};
use anyhow::Result;
use rollups_interface::l2::{bank::BankOperations, storage::StorageOperations};
use solana_sdk::{account::ReadableAccount, pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_svm::account_loader::LoadedTransaction;

use super::mock::system_account;

#[tokio::test]
async fn init_with_all_default_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut store = RollupStorage::new(GlobalConfig::new_temp(&ledger_path)?)?;
    store.init()?;

    assert!(store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    let accounts_status = store.all_accounts_status()?;
    assert_eq!(accounts_status.num_accounts, 222);
    store.close().await?;

    // create store again
    let mut store = RollupStorage::new(GlobalConfig::new(&ledger_path)?)?;
    store.init()?;
    assert!(!store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    let accounts_status = store.all_accounts_status()?;
    assert_eq!(accounts_status.num_accounts, 222);

    Ok(())
}

#[test]
fn init_with_given_config_works() -> Result<()> {
    let mut store = RollupStorage::new(GlobalConfig::new(&Path::new("data/config/ledger"))?)?;
    store.init()?;

    assert!(!store.allow_init_from_scratch());
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    assert_eq!(store.current_height(), 0);
    Ok(())
}

#[tokio::test]
async fn storage_basic_process_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut store = RollupStorage::new(GlobalConfig::new_temp(&ledger_path)?)?;
    store.init()?;

    store.set_snapshot_interval(1);

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
    .for_each(|(key, data)| store.insert_account(key, data));

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
        &results.loaded_transactions[0].as_ref().unwrap(),
    );
    assert_result_balance(
        alice,
        Some(INIT_AMOUNT - BOB_PLUS),
        &results.loaded_transactions[0].as_ref().unwrap(),
    );
    // assert second transaction result
    assert_result_balance(
        alice,
        None, // alice balance not changed
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        bob,
        Some(INIT_AMOUNT - BOB_MINUS), // TODO: should be INIT_AMOUNT + BOB_PLUS - BOB_MINUS ?
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    assert_result_balance(
        charlie,
        Some(INIT_AMOUNT + BOB_MINUS),
        &results.loaded_transactions[1].as_ref().unwrap(),
    );
    // after process balance not changed
    assert_eq!(store.balance(&alice), INIT_AMOUNT);
    assert_eq!(store.balance(&bob), INIT_AMOUNT);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT);

    // 3. commit
    store
        .commit(
            TransactionsResultWrapper { output: results },
            &CommitBatch::new(create_svm_transactions(&raw_txs).into()),
        )
        .await?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));

    assert_eq!(store.balance(&alice), INIT_AMOUNT - BOB_PLUS);
    // assert_eq!(store.balance(&bob), INIT_AMOUNT + BOB_PLUS - BOB_MINUS);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT + BOB_MINUS);

    // 5. save and close
    store.force_save().await?;
    // store.snapshot(None)?;
    store.close().await?;

    // 6.  again
    let mut store = RollupStorage::new(GlobalConfig::new(&ledger_path)?)?;
    store.init()?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    assert_eq!(store.balance(&alice), INIT_AMOUNT - BOB_PLUS);
    // assert_eq!(store.balance(&bob), INIT_AMOUNT + BOB_PLUS - BOB_MINUS);
    assert_eq!(store.balance(&charlie), INIT_AMOUNT + BOB_MINUS);

    Ok(())
}

fn assert_result_balance(account: Pubkey, lamports: Option<u64>, tx: &LoadedTransaction) {
    match tx.accounts.iter().find(|key| key.0 == account) {
        Some(recipient_data) => assert_eq!(recipient_data.1.lamports(), lamports.unwrap()),
        None => assert!(lamports.is_none()),
    }
}
