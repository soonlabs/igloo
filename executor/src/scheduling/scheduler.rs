use crate::scheduling::status_slicing::SvmWorkerSlicingStatus;
use crate::scheduling::ScheduledTransaction;
use crate::SanitizedTransactions;
use crossbeam_channel::Receiver;
use solana_prio_graph_scheduler::prio_graph_scheduler::{PrioGraphScheduler, SchedulingSummary};
use solana_prio_graph_scheduler::scheduler_messages::{TransactionBatchId, TransactionId};
use solana_sdk::inner_instruction::InnerInstruction;
use solana_sdk::transaction::SanitizedTransaction;
use std::sync::mpsc::Sender;

/// A Scheduler is a single-thread centralized scheduling thread.
///
/// It will be initialized with N task sending channels and a task callback channel,
/// with normally an inner scheduling status machine.
/// Workflow just like below:
///              -> Task channel1 -> [worker1] -> Task finish callback ->
///             |                       ...                             |
/// Scheduler --   Task channelK -> [workerK] -> Task finish callback -> Scheduler
///            |                        ...                            |
///             -> Task channelN -> [workerN] -> Task finish callback ->
pub trait Scheduler {
    fn new(
        schedule_task_receivers: Vec<Sender<SchedulingBatch>>,
        task_finish_receiver: Receiver<SchedulingBatchResult>,
    ) -> Self;

    fn schedule_batch(&mut self, txs: SchedulingBatch) {}
}

/// Scheduling unit.
pub struct SchedulingBatch {
    pub batch_id: TransactionBatchId,
    pub ids: Vec<TransactionId>,
    pub transactions: SanitizedTransactions,
}

/// The scheduling result from worker one time.
/// Since the `SchedulingBatch` will be dispute to different subset to multi workers,
/// the `SchedulingBatchResult` is not 1-1 with SchedulingBatch.
/// One `batch_id` may occur mostly `num_of_worker` times.
pub struct SchedulingBatchResult {
    // workload.
    pub batch: SchedulingBatch,
    // time slice status for this batch job.
    pub status_summary: Vec<SvmWorkerSlicingStatus>,
}

/// NoLockScheduler is a dummy scheduler that does not lock any resources.
pub struct NoLockScheduler {}

pub struct SolanaPrioGraphScheduler {
    inner: PrioGraphScheduler<ScheduledTransaction>,
}

impl Scheduler for SolanaPrioGraphScheduler {
    fn new(
        schedule_task_receivers: Vec<Sender<SchedulingBatch>>,
        task_finish_receiver: Receiver<SchedulingBatchResult>,
    ) -> Self {
        let scheduler = PrioGraphScheduler::<ScheduledTransaction>::new();
    }

    fn schedule_batch(&mut self, txs: SchedulingBatch) {
        todo!()
    }
}
