use anyhow::Result;
use solana_accounts_db::{
    accounts_db::{self, ACCOUNTS_DB_CONFIG_FOR_TESTING},
    accounts_index::AccountSecondaryIndexes,
};
use solana_runtime::{
    bank::Bank,
    snapshot_bank_utils,
    snapshot_utils::{self, create_tmp_accounts_dir_for_tests},
};
use solana_sdk::{
    clock::Slot, pubkey::Pubkey, signature::Keypair, signer::Signer, system_transaction,
    transaction::SanitizedTransaction,
};
use solana_svm::runtime_config::RuntimeConfig;
use std::time::{Duration, Instant};

use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::processor::process_transfers_ex,
    RollupStorage,
};

#[tokio::test]
async fn init_from_snapshot_works() -> Result<()> {
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;
    config
        .storage
        .snapshot_config
        .full_snapshot_archive_interval_slots = 1; // set snapshot interval to 1
    config.storage.wait_snapshot_complete = true;
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
        .map(SanitizedTransaction::from_transaction_for_tests)
        .collect::<Vec<_>>();
    let results = process_transfers_ex(&store, origin_txs.clone());

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
    store.close().await?;

    // 4. open with snapshot
    assert!(ledger_path.join("snapshots/1/1").exists());
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs.clone();
    let mut store = RollupStorage::new(config)?;
    store.init()?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 1);
    assert_eq!(store_height, Some(1));
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - TO_CHARLIE - FEE
    );
    // TODO: check why bob balance is not `BOB_INIT_BALANCE - TO_DAVE - FEE` ?
    assert_eq!(store.balance(&bob.pubkey()), BOB_INIT_BALANCE - TO_DAVE);
    assert_eq!(store.balance(&charlie), TO_CHARLIE);
    assert_eq!(store.balance(&dave), TO_DAVE);
    store.close().await?;

    // 5. open without snapshot
    let mut config = GlobalConfig::new(&ledger_path)?;
    config.keypairs = keypairs;
    config.storage.snapshot_config = Default::default(); // disable snapshot
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
    store.close().await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Takes a long time to run"]
async fn test_snapshots_with_background_services() -> Result<()> {
    const SET_ROOT_INTERVAL_SLOTS: Slot = 2;
    const BANK_SNAPSHOT_INTERVAL_SLOTS: Slot = SET_ROOT_INTERVAL_SLOTS * 2;
    const INCREMENTAL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS: Slot = BANK_SNAPSHOT_INTERVAL_SLOTS * 3;
    const FULL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS: Slot =
        INCREMENTAL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS * 5;
    const LAST_SLOT: Slot =
        FULL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS * 3 + INCREMENTAL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS * 2;

    // Maximum amount of time to wait for each snapshot archive to be created.
    // This should be enough time, but if it times-out in CI, try increasing it.
    const MAX_WAIT_DURATION: Duration = Duration::from_secs(10);

    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;

    let snapshot_config = &mut config.storage.snapshot_config;
    snapshot_config.full_snapshot_archive_interval_slots = FULL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS;
    snapshot_config.incremental_snapshot_archive_interval_slots =
        INCREMENTAL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS;
    let mut store = RollupStorage::new(config)?;
    store.set_snapshot_interval(BANK_SNAPSHOT_INTERVAL_SLOTS);
    store.init()?;

    let snapshot_config = &store.config.storage.snapshot_config;
    let mut last_full_snapshot_slot = None;
    let mut last_incremental_snapshot_slot = None;
    let mint_keypair = store.config.keypairs.mint_keypair.as_ref().unwrap();
    for slot in 1..=LAST_SLOT {
        // Make a new bank and process some transactions
        {
            let bank = Bank::new_from_parent(
                store.bank_forks.read().unwrap().get(slot - 1).unwrap(),
                &Pubkey::default(),
                slot,
            );
            let bank = store
                .bank_forks
                .write()
                .unwrap()
                .insert(bank)
                .clone_without_scheduler();

            let key = solana_sdk::pubkey::new_rand();
            let tx =
                system_transaction::transfer(mint_keypair, &key, 1000000, bank.last_blockhash());
            assert_eq!(bank.process_transaction(&tx), Ok(()));

            let key = solana_sdk::pubkey::new_rand();
            let tx =
                system_transaction::transfer(mint_keypair, &key, 1000000, bank.last_blockhash());
            assert_eq!(bank.process_transaction(&tx), Ok(()));

            while !bank.is_complete() {
                bank.register_unique_tick();
            }
        }

        // Call `BankForks::set_root()` to cause snapshots to be taken
        if slot % SET_ROOT_INTERVAL_SLOTS == 0 {
            store
                .bank_forks
                .write()
                .unwrap()
                .set_root(
                    slot,
                    &store.background_service.accounts_background_request_sender,
                    None,
                )
                .unwrap();
        }

        // If a snapshot should be taken this slot, wait for it to complete
        if slot % FULL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS == 0 {
            let timer = Instant::now();
            while snapshot_utils::get_highest_full_snapshot_archive_slot(
                &snapshot_config.full_snapshot_archives_dir,
            ) != Some(slot)
            {
                assert!(
                    timer.elapsed() < MAX_WAIT_DURATION,
                    "Waiting for full snapshot {slot} exceeded the {MAX_WAIT_DURATION:?} maximum wait duration!",
                );
                std::thread::sleep(Duration::from_secs(1));
            }
            last_full_snapshot_slot = Some(slot);
        } else if slot % INCREMENTAL_SNAPSHOT_ARCHIVE_INTERVAL_SLOTS == 0
            && last_full_snapshot_slot.is_some()
        {
            let timer = Instant::now();
            while snapshot_utils::get_highest_incremental_snapshot_archive_slot(
                &snapshot_config.incremental_snapshot_archives_dir,
                last_full_snapshot_slot.unwrap(),
            ) != Some(slot)
            {
                assert!(
                    timer.elapsed() < MAX_WAIT_DURATION,
                    "Waiting for incremental snapshot {slot} exceeded the {MAX_WAIT_DURATION:?} maximum wait duration!",
                );
                std::thread::sleep(Duration::from_secs(1));
            }
            last_incremental_snapshot_slot = Some(slot);
        }
    }

    // Load the snapshot and ensure it matches what's in BankForks
    let (_tmp_dir, temporary_accounts_dir) = create_tmp_accounts_dir_for_tests();
    let (deserialized_bank, ..) = snapshot_bank_utils::bank_from_latest_snapshot_archives(
        &snapshot_config.bank_snapshots_dir,
        &snapshot_config.full_snapshot_archives_dir,
        &snapshot_config.incremental_snapshot_archives_dir,
        &[temporary_accounts_dir],
        &store.config.genesis,
        &RuntimeConfig::default(),
        None,
        None,
        AccountSecondaryIndexes::default(),
        None,
        accounts_db::AccountShrinkThreshold::default(),
        false,
        false,
        false,
        false,
        Some(ACCOUNTS_DB_CONFIG_FOR_TESTING),
        None,
        store.exit.clone(),
    )
    .unwrap();
    deserialized_bank.wait_for_initial_accounts_hash_verification_completed_for_tests();

    assert_eq!(
        deserialized_bank.slot(),
        last_incremental_snapshot_slot.unwrap()
    );
    assert_eq!(
        &deserialized_bank,
        store
            .bank_forks
            .read()
            .unwrap()
            .get(deserialized_bank.slot())
            .unwrap()
            .as_ref()
    );

    // Stop the background services, ignore any errors
    store.close().await?;

    Ok(())
}
