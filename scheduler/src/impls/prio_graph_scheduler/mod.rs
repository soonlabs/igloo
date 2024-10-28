use crate::scheduler::Scheduler;
use crate::scheduler_messages::{SchedulingBatch, SchedulingBatchResult};
use crossbeam_channel::{Receiver, Sender};

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
