use std::sync::Arc;

use anyhow::Result;
use solana_runtime::bank::Bank;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_transaction,
    transaction::SanitizedTransaction,
};

use crate::{
    blockstore::txs::CommitBatch,
    config::GlobalConfig,
    execution::TransactionsResultWrapper,
    init::default::{DEFAULT_MINT_LAMPORTS, DEFAULT_VALIDATOR_LAMPORTS},
    tests::mock::processor::process_transfers_ex,
    RollupStorage,
};

#[tokio::test]
async fn reorg_works() -> Result<()> {
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

    /// slot 4    (E) *
    ///               |
    /// slot 3        |    * (D) <-- root, from set_root()
    ///               |    |
    /// slot 2    (C) *    |
    ///                \   |
    /// slot 1          \  * (B) <-- highest confirmed root
    ///                  \ |
    /// slot 0             * (A)
    const FEE: u64 = 5000;

    let bank0 = store.bank.clone();
    assert_eq!(bank0.slot(), 0);

    // Slot 1
    // FIXME: assert_eq! failed if transfer amount is 890_880 or less,
    // FIXME: because 890_880 is minimum rent exempt amount,
    // FIXME: should check this situation before `transfer` instruction.
    const SLOT1_TO_CHARLIE: u64 = 2_000_000;
    const SLOT1_TO_DAVE: u64 = 1_000_000;
    store.bump()?;
    let bank1 = store.bank.clone();
    assert_eq!(bank1.slot(), 1);
    assert_eq!(bank1.parent_slot(), 0);
    new_slot_with_two_txs(
        bank1.clone(),
        &mut store,
        alice.clone(),
        charlie,
        SLOT1_TO_CHARLIE,
        bob.clone(),
        dave,
        SLOT1_TO_DAVE,
    )
    .await?;
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - SLOT1_TO_CHARLIE - FEE
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT1_TO_DAVE - FEE
    );
    assert_eq!(store.balance(&charlie), SLOT1_TO_CHARLIE);
    assert_eq!(store.balance(&dave), SLOT1_TO_DAVE);

    // Slot 2
    const SLOT2_TO_CHARLIE: u64 = 2_000_000;
    const SLOT2_TO_DAVE: u64 = 1_000_000;
    let bank2 = store.new_slot_with_parent(bank0.clone(), 2)?.clone();
    assert_eq!(bank2.slot(), 2);
    assert_eq!(bank2.parent_slot(), 0);
    new_slot_with_two_txs(
        bank2.clone(),
        &mut store,
        alice.clone(),
        charlie,
        SLOT2_TO_CHARLIE,
        bob.clone(),
        dave,
        SLOT2_TO_DAVE,
    )
    .await?;
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - SLOT2_TO_CHARLIE - FEE
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT2_TO_DAVE - FEE
    );
    assert_eq!(store.balance(&charlie), SLOT2_TO_CHARLIE);
    assert_eq!(store.balance(&dave), SLOT2_TO_DAVE);

    // Slot 3
    const SLOT3_TO_CHARLIE: u64 = 2_000_000;
    const SLOT3_TO_DAVE: u64 = 1_000_000;
    let bank3 = store.new_slot_with_parent(bank1.clone(), 3)?.clone();
    assert_eq!(bank3.slot(), 3);
    assert_eq!(bank3.parent_slot(), 1);
    // + FEE here because bob is the leader of Slot 1, so it has the 50% tx fee of that Slot.
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT1_TO_DAVE - FEE + FEE
    );
    new_slot_with_two_txs(
        bank3,
        &mut store,
        alice.clone(),
        charlie,
        SLOT3_TO_CHARLIE,
        bob.clone(),
        dave,
        SLOT3_TO_DAVE,
    )
    .await?;
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - SLOT1_TO_CHARLIE - SLOT3_TO_CHARLIE - FEE * 2
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT1_TO_DAVE - SLOT3_TO_DAVE - FEE
    );
    assert_eq!(store.balance(&charlie), SLOT1_TO_CHARLIE + SLOT3_TO_CHARLIE);
    assert_eq!(store.balance(&dave), SLOT1_TO_DAVE + SLOT3_TO_DAVE);

    // Slot 4
    const SLOT4_TO_CHARLIE: u64 = 2_000_000;
    const SLOT4_TO_DAVE: u64 = 1_000_000;
    let bank4 = store.new_slot_with_parent(bank2.clone(), 4)?.clone();
    assert_eq!(bank4.slot(), 4);
    assert_eq!(bank4.parent_slot(), 2);
    // + FEE here because bob is the leader of Slot 2, so it has the 50% tx fee of that Slot.
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT2_TO_DAVE - FEE + FEE
    );

    new_slot_with_two_txs(
        bank4,
        &mut store,
        alice.clone(),
        charlie,
        SLOT4_TO_CHARLIE,
        bob.clone(),
        dave,
        SLOT4_TO_DAVE,
    )
    .await?;
    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - SLOT2_TO_CHARLIE - SLOT4_TO_CHARLIE - FEE * 2,
    );
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT2_TO_DAVE - SLOT4_TO_DAVE - FEE
    );

    assert_eq!(store.current_height(), 4);
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 4);
    assert_eq!(store_height, Some(4));

    // reorg to slot 3 with highest confirmed root of slot 1
    let removed = store.reorg(3, Some(1))?;
    assert_eq!(removed.len(), 3);
    let mut remove_slots = removed.iter().map(|bank| bank.slot()).collect::<Vec<_>>();
    remove_slots.sort();
    assert_eq!(remove_slots, vec![0, 2, 4]);
    assert_eq!(store.current_height(), 3);
    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 3);
    assert_eq!(store_height, Some(4));

    assert_eq!(
        store.balance(&alice.pubkey()),
        ALICE_INIT_BALANCE - SLOT1_TO_CHARLIE - SLOT3_TO_CHARLIE - FEE * 2
    );
    // + FEE * 2 here because bob is the leader of Slot 1 & Slot 3, so it has the 50% tx fee of that 2 Slots.
    assert_eq!(
        store.balance(&bob.pubkey()),
        BOB_INIT_BALANCE - SLOT1_TO_DAVE - SLOT3_TO_DAVE - FEE * 2 + FEE * 2
    );
    assert_eq!(store.balance(&charlie), SLOT1_TO_CHARLIE + SLOT3_TO_CHARLIE);
    assert_eq!(store.balance(&dave), SLOT1_TO_DAVE + SLOT3_TO_DAVE);

    let bank_forks = store.bank_forks.read().unwrap();
    assert!(bank_forks.get(0).is_none());
    assert!(bank_forks.get(1).is_some());
    assert!(bank_forks.get(2).is_none());
    assert!(bank_forks.get(3).is_some());
    assert!(bank_forks.get(4).is_none());

    let blockstore = store.blockstore.clone();
    assert!(blockstore.get_slot_entries(4, 0).is_err());
    assert!(!blockstore.get_slot_entries(3, 0).unwrap().is_empty());
    assert!(blockstore.get_slot_entries(2, 0).is_err());
    assert!(!blockstore.get_slot_entries(1, 0).unwrap().is_empty());
    // slot 0 removed in bank but should not set to dead
    assert!(!blockstore.get_slot_entries(0, 0).unwrap().is_empty());

    // slot 4 and slot 2 in blockstore set to dead but still exists
    let result = blockstore
        .get_slot_entries_with_shred_info(4, 0, true)
        .unwrap();
    assert!(!result.0.is_empty());
    let result = blockstore
        .get_slot_entries_with_shred_info(2, 0, true)
        .unwrap();
    assert!(!result.0.is_empty());

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn new_slot_with_two_txs(
    bank: Arc<Bank>,
    store: &mut RollupStorage,
    tx1_from: Arc<Keypair>,
    tx1_to: Pubkey,
    tx1_amount: u64,
    tx2_from: Arc<Keypair>,
    tx2_to: Pubkey,
    tx2_amount: u64,
) -> anyhow::Result<()> {
    let origin_txs = [
        system_transaction::transfer(&tx1_from, &tx1_to, tx1_amount, bank.last_blockhash()),
        system_transaction::transfer(&tx2_from, &tx2_to, tx2_amount, bank.last_blockhash()),
    ]
    .into_iter()
    .map(SanitizedTransaction::from_transaction_for_tests)
    .collect::<Vec<_>>();
    let results = process_transfers_ex(store, origin_txs.clone());
    store
        .commit(
            vec![TransactionsResultWrapper { output: results }],
            vec![CommitBatch::new(origin_txs.clone().into())],
        )
        .await?;

    Ok(())
}
