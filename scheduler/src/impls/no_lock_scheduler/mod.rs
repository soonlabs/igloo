use crate::impls::prio_graph_scheduler::scheduler_error::SchedulerError;
use crate::scheduler::Scheduler;
use crate::scheduler_messages::{SchedulingBatch, SchedulingBatchResult};
use crossbeam_channel::{Receiver, Sender};

/// NoLockScheduler is a dummy scheduler that does not lock any resources.
pub struct NoLockScheduler {
    thread_num: usize,
    task_senders: Vec<Sender<SchedulingBatch>>,
}

impl Scheduler for NoLockScheduler {
    fn new(
        schedule_task_senders: Vec<Sender<SchedulingBatch>>,
        _: Receiver<SchedulingBatchResult>,
    ) -> Self {
        Self {
            thread_num: schedule_task_senders.len(),
            task_senders: schedule_task_senders,
        }
    }

    fn schedule_batch(&mut self, txs: SchedulingBatch) -> Result<(), SchedulerError> {
        let exec_batch = 64;
        txs.transactions
            .chunks(exec_batch)
            .enumerate()
            .for_each(|(i, chunk)| {
                let worker_id =
                    (txs.batch_id.value() as usize % self.thread_num + i) % self.thread_num;
                let batch = SchedulingBatch {
                    batch_id: txs.batch_id,
                    ids: txs.ids[i * exec_batch..(i + 1) * exec_batch].to_vec(),
                    transactions: chunk.to_vec(),
                    max_ages: vec![],
                };
                self.task_senders[worker_id].send(batch).unwrap();
            });
        Ok(())
    }

    fn receive_complete(&mut self) -> Result<(), SchedulerError> {
        Ok(())
    }
}
