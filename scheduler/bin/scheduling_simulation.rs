use crossbeam_channel::{unbounded, Receiver, Sender};
use igloo_executor::processor::TransactionProcessor;
use igloo_scheduler::id_generator::IdGenerator;
use igloo_scheduler::impls::prio_graph_scheduler::PrioGraphSchedulerWrapper;
use igloo_scheduler::scheduler::Scheduler;
use igloo_scheduler::scheduler_messages::{MaxAge, SchedulingBatch, SchedulingBatchResult};
use igloo_scheduler::status_slicing::{
    calculate_thread_load_summary, SvmWorkerSlicingStatus, WorkerStatusUpdate,
};
use igloo_scheduler::stopwatch::StopWatch;
use igloo_storage::{config::GlobalConfig, RollupStorage};
use igloo_verifier::settings::{Settings, Switchs};
use itertools::Itertools;
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
use std::time::{SystemTime, UNIX_EPOCH};

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

const TOTAL_TX_NUM: usize = 1024 * 4;
const TOTAL_WORKER_NUM: usize = 4;
// each tx need 2 unique accounts.
const NUM_ACCOUNTS: usize = TOTAL_TX_NUM * 2;
// initial account balance: 100 SOL.
const ACCOUNT_BALANCE: u64 = 100_000_000_000;
// batch size of each call of scheduler.
const SCHEDULER_BATCH_SIZE: usize = 2048;

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
    thread_id: usize,
    receiver: Receiver<SchedulingBatch>,
    store: Arc<RollupStorage>,
    settings: Settings,
    status_sender: Sender<WorkerStatusUpdate>,
    completed_sender: Sender<SchedulingBatchResult>,
) -> Result<usize, E> {
    let bank_processor = TransactionProcessor::new(store.current_bank(), settings);
    let mut success_count = 0;
    let mut idle_start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    while let Ok(scheduled_txs) = receiver.recv() {
        // Calculate and send idle status before processing
        let active_start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let idle_status = SvmWorkerSlicingStatus::new_idle(idle_start, active_start);
        status_sender.send(WorkerStatusUpdate {
            thread_id,
            status: idle_status,
        })?;
        // Process transactions
        let execute_result = bank_processor.process(Cow::Borrowed(&scheduled_txs.transactions))?;
        success_count += execute_result
            .execution_results
            .iter()
            .filter(|x| x.was_executed_successfully())
            .count();

        let active_end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let active_status = SvmWorkerSlicingStatus::new_active(active_start, active_end);
        if let Err(e) = status_sender.send(WorkerStatusUpdate {
            thread_id,
            status: active_status,
        }) {
            eprintln!("send status error: {:?}", e);
        }

        // TODO retryable_indexes logic
        let result = SchedulingBatchResult {
            batch: scheduled_txs,
            retryable_indexes: vec![],
        };

        // it's ok to ignore send error.
        // because error is handled by the scheduler.
        // if scheduler exits, means all task is scheduled.
        // no need to maintain channel now.
        let _ = completed_sender.send(result);

        // Update idle_start for next iteration
        idle_start = active_end;
    }

    Ok(success_count)
}

fn main() -> Result<(), E> {
    let mut stopwatch = StopWatch::new("scheduling_simulation");

    // workload channel.
    let (senders, receivers): (Vec<Sender<SchedulingBatch>>, Vec<Receiver<SchedulingBatch>>) =
        (0..TOTAL_WORKER_NUM).map(|_| unbounded()).unzip();

    // thread slice_status update channel
    let (status_sender, status_receiver) = unbounded();

    // transaction completed channel
    let (completed_sender, completed_receiver) = unbounded();

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
        .map(|chunk| {
            mocking_transfer_tx(&chunk[0].0, &chunk[1].0.pubkey(), 1e9 as u64, recent_hash)
        })
        .collect::<Result<Vec<_>, _>>()?;
    stopwatch.click("tx generation");

    // Start worker threads
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
        .enumerate()
        .map(|(i, receiver)| {
            let store_clone = Arc::clone(&store);
            let settings_clone = settings.clone();
            thread::spawn({
                let ss = status_sender.clone();
                let cs = completed_sender.clone();
                move || worker_process(i, receiver, store_clone, settings_clone, ss, cs)
            })
        })
        .collect();

    let mut batch_id_gen = IdGenerator::default();
    let mut tx_id_gen = IdGenerator::default();

    let mut scheduler = PrioGraphSchedulerWrapper::new(senders.clone(), completed_receiver);
    for chunk in transfer_txs
        .into_iter()
        .chunks(SCHEDULER_BATCH_SIZE)
        .into_iter()
        .map(|chunk| chunk.collect())
        .map(|transactions: Vec<_>| {
            let len = transactions.len();
            let ids = transactions
                .iter()
                .map(|_| tx_id_gen.gen())
                .collect::<Vec<_>>();
            SchedulingBatch {
                batch_id: batch_id_gen.gen(),
                ids,
                transactions,
                max_ages: vec![MaxAge::default(); len],
            }
        })
        .collect::<Vec<SchedulingBatch>>()
    {
        scheduler.schedule_batch(chunk)?;
        scheduler.receive_complete()?;
    }

    // Close senders to signal workers to finish
    drop(senders);
    drop(scheduler);
    drop(status_sender);

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
    let mut all_status_point = Vec::with_capacity(1024 * 256);
    while let Ok(point) = status_receiver.recv() {
        all_status_point.push(point);
    }

    println!("time stat: {}", stopwatch.summary());

    // Sort all_status_point by thread_id and then by start time
    all_status_point.sort_by(|a, b| {
        a.thread_id.cmp(&b.thread_id).then_with(|| {
            let a_start = match &a.status {
                SvmWorkerSlicingStatus::Active { start, .. } => *start,
                SvmWorkerSlicingStatus::Idle { start, .. } => *start,
            };
            let b_start = match &b.status {
                SvmWorkerSlicingStatus::Active { start, .. } => *start,
                SvmWorkerSlicingStatus::Idle { start, .. } => *start,
            };
            a_start.cmp(&b_start)
        })
    });

    // Print sorted worker status updates
    println!(
        "thread load: {:?}",
        calculate_thread_load_summary(all_status_point.as_slice())
    );

    Ok(())
}
