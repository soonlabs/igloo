pub mod in_flight_tracker;
pub mod read_write_account_set;
pub mod scheduler;
pub mod scheduler_error;
pub mod scheduler_metrics;
pub mod thread_aware_account_locks;
pub mod transaction_priority_id;
pub mod transaction_state;
pub mod transaction_state_container;

use crate::impls::prio_graph_scheduler::scheduler::PrioGraphScheduler;
use crate::impls::prio_graph_scheduler::scheduler_error::SchedulerError;
use crate::impls::prio_graph_scheduler::transaction_state::SanitizedTransactionTTL;
use crate::impls::prio_graph_scheduler::transaction_state_container::TransactionStateContainer;
use crate::scheduler::Scheduler;
use crate::scheduler_messages::{SchedulingBatch, SchedulingBatchResult};
use crossbeam_channel::{Receiver, Sender};

pub const TARGET_NUM_TRANSACTIONS_PER_BATCH: usize = 128;

pub struct PrioGraphSchedulerWrapper {
    inner: PrioGraphScheduler,
    container: TransactionStateContainer,
}

impl Scheduler for PrioGraphSchedulerWrapper {
    fn new(
        schedule_task_senders: Vec<Sender<SchedulingBatch>>,
        task_finished_receivers: Receiver<SchedulingBatchResult>,
    ) -> Self {
        Self {
            inner: PrioGraphScheduler::new(schedule_task_senders, task_finished_receivers),
            container: TransactionStateContainer::with_capacity(10240),
        }
    }

    fn schedule_batch(&mut self, mut txs: SchedulingBatch) -> Result<(), SchedulerError> {
        for ((tx, tx_id), max_age) in txs
            .transactions
            .drain(..)
            .zip(txs.ids.drain(..))
            .zip(txs.max_ages.drain(..))
        {
            self.container.insert_new_transaction(
                tx_id,
                SanitizedTransactionTTL {
                    transaction: tx,
                    max_age,
                },
                // TODO migrate priority
                0,
                100,
            );
        }

        self.inner.schedule(
            &mut self.container,
            // TODO: migrate pre-filter transactions
            |_, result| result.fill(true),
            |_| true,
        )?;
        Ok(())
    }

    fn receive_complete(&mut self) -> Result<(), SchedulerError> {
        // TODO metrics logic
        self.inner.receive_completed(&mut self.container)?;
        Ok(())
    }
}
