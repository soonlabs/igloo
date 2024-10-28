use crate::scheduler_messages::{SchedulingBatch, SchedulingBatchResult};
use crossbeam_channel::{Receiver, Sender};

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
        schedule_task_senders: Vec<Sender<SchedulingBatch>>,
        task_finished_receivers: Receiver<SchedulingBatchResult>,
    ) -> Self;

    fn schedule_batch(&mut self, txs: SchedulingBatch);
}
