use crossbeam_channel::{unbounded, Receiver, Sender};
use igloo_executor::processor::TransactionProcessor;
use igloo_executor::scheduling::stopwatch::StopWatch;
use igloo_executor::scheduling::ScheduledTransaction;
use igloo_storage::blockstore::txs::CommitBatch;
use igloo_storage::execution::TransactionsResultWrapper;
use igloo_storage::{config::GlobalConfig, RollupStorage};
use igloo_verifier::settings::{Settings, Switchs};
use solana_program::hash::Hash;
use solana_sdk::account::AccountSharedData;
use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, system_transaction,
};
use std::borrow::Cow;
use std::error::Error;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use std::vec;

type E = Box<dyn Error + Send + Sync>;

/// Generate a mocked transfer transaction from one account to another
///
/// # Parameters
/// * `from` - The `Keypair` of the sender account
/// * `to` - The `Pubkey` of the recipient account
/// * `amount` - The amount of lamports to transfer
///
/// # Returns
/// A `Result` containing a `SanitizedTransaction` representing the transfer, or an error
fn mocking_transfer_tx(
    from: &Keypair,
    to: &Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<SanitizedTransaction, E> {
    let transaction = system_transaction::transfer(from, to, amount, recent_blockhash);
    Ok(SanitizedTransaction::from_transaction_for_tests(
        transaction,
    ))
}

const TOTAL_TX_NUM: usize = 1024 * 1;
const TOTAL_WORKER_NUM: usize = 4;
// each tx need 2 unique accounts.
const NUM_ACCOUNTS: usize = TOTAL_TX_NUM * 2;
// initial account balance: 100 SOL.
const ACCOUNT_BALANCE: u64 = 100_000_000_000;

/// Worker process function that receives ScheduledTransactions and processes them
///
/// # Parameters
/// * `receiver` - The channel receiver for incoming ScheduledTransactions
/// * `store` - The RollupStorage instance
/// * `settings` - The Settings for the TransactionProcessor
///
/// # Returns
/// A Result containing the number of successfully processed transactions, or an error
fn worker_process(
    receiver: Receiver<Vec<ScheduledTransaction>>,
    store: Arc<RollupStorage>,
    settings: Settings,
) -> Result<usize, E> {
    let bank_processor = TransactionProcessor::new(store.current_bank(), settings);
    let mut success_count = 0;

    while let Ok(scheduled_txs) = receiver.recv() {
        let sanitized_txs: Vec<SanitizedTransaction> =
            scheduled_txs.into_iter().map(|st| st.transaction).collect();

        let execute_result = bank_processor.process(Cow::Borrowed(&sanitized_txs))?;
        success_count += execute_result
            .execution_results
            .iter()
            .filter(|x| x.was_executed_successfully())
            .count();
    }

    Ok(success_count)
}

fn main() -> Result<(), E> {
    let mut stopwatch = StopWatch::new();

    // workload channel.
    let (senders, receivers): (
        Vec<Sender<Vec<ScheduledTransaction>>>,
        Vec<Receiver<Vec<ScheduledTransaction>>>,
    ) = (0..TOTAL_WORKER_NUM).map(|_| unbounded()).unzip();

    let accounts: Vec<_> = (0..NUM_ACCOUNTS)
        .map(|_| (Keypair::new(), ACCOUNT_BALANCE))
        .collect();
    stopwatch.click("account initialization");

    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;
    config.allow_default_genesis = true;

    // Insert accounts into genesis
    for (keypair, balance) in &accounts {
        config.genesis.add_account(
            keypair.pubkey(),
            AccountSharedData::new(*balance, 0, &system_program::id()),
        );
    }

    let mut store = RollupStorage::new(config)?;
    store.init()?;
    store.bump()?;
    let store = Arc::new(store);
    stopwatch.click("storage initialization");

    let recent_hash = store.current_bank().last_blockhash();
    let transfer_txs = accounts
        .chunks(2)
        .enumerate()
        .map(|(_, chunk)| {
            mocking_transfer_tx(&chunk[0].0, &chunk[1].0.pubkey(), 1e9 as u64, recent_hash)
        })
        .collect::<Result<Vec<_>, _>>()?;
    stopwatch.click("tx generation");

    // Create worker threads
    let settings = Settings {
        max_age: Default::default(),
        switchs: Switchs {
            tx_sanity_check: false,
            txs_conflict_check: true,
        },
        fee_structure: Default::default(),
    };

    let worker_handles: Vec<_> = receivers
        .into_iter()
        .map(|receiver| {
            let store_clone = Arc::clone(&store);
            let settings_clone = settings.clone();
            thread::spawn(move || worker_process(receiver, store_clone, settings_clone))
        })
        .collect();

    // Distribute transactions to workers
    let chunk_size = transfer_txs.len() / TOTAL_WORKER_NUM;
    for (i, chunk) in transfer_txs.chunks(chunk_size).enumerate() {
        let scheduled_txs: Vec<ScheduledTransaction> = chunk
            .iter()
            .map(|tx| ScheduledTransaction {
                id: i as u64,
                priority: 0,
                transaction: tx.clone(),
            })
            .collect();
        senders[i].send(scheduled_txs).unwrap();
    }

    // Close senders to signal workers to finish
    drop(senders);

    // Wait for workers to finish and collect results
    let total_success_count: usize = worker_handles
        .into_iter()
        .map(|handle| handle.join().unwrap().unwrap())
        .sum();
    stopwatch.click(format!("tx execution(done: {})", total_success_count));

    // let commit_start = Instant::now();
    // let result = store.commit_block(
    //     vec![TransactionsResultWrapper {
    //         output: Default::default(), // Note: This needs to be updated with actual results
    //     }],
    //     vec![CommitBatch::new(transfer_txs.into())],
    // )?;
    // let commit_duration = commit_start.elapsed();
    // println!("Block commit time: {:?}", commit_duration);

    println!("{}", stopwatch.summary());
    Ok(())
}
