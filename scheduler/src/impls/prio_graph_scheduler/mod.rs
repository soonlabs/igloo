pub mod in_flight_tracker;
pub mod prio_graph_scheduler;
pub mod read_write_account_set;
pub mod scheduler_error;
pub mod scheduler_metrics;
pub mod thread_aware_account_locks;
pub mod transaction_priority_id;
pub mod transaction_state_container;
pub mod transaction_state;

use crate::scheduler::Scheduler;
use crate::scheduler_messages::{SchedulingBatch, SchedulingBatchResult};
use crossbeam_channel::{Receiver, Sender};

pub const TARGET_NUM_TRANSACTIONS_PER_BATCH: i32 = 128;

pub struct PrioGraphSchedulerWrapper {}

impl Scheduler for PrioGraphSchedulerWrapper {
    fn new(
        schedule_task_senders: Vec<Sender<SchedulingBatch>>,
        task_finished_receivers: Receiver<SchedulingBatchResult>,
    ) -> Self {
        todo!()
    }

    fn schedule_batch(&mut self, txs: SchedulingBatch) {
        todo!()
    }

    fn receive_complete(&mut self, receipt: SchedulingBatchResult) {
        todo!()
    }
}
