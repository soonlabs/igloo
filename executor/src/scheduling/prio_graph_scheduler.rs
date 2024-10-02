use crate::scheduling::seq_id_generator::SeqIdGenerator;
use crate::scheduling::thread_aware_account_locks::{ThreadAwareAccountLocks, ThreadSet};
use crossbeam_channel::{Receiver, Sender};
use rand::prelude::IteratorRandom;
use solana_sdk::transaction::SanitizedTransaction;
use crate::scheduling::ScheduledTransaction;

// Defines the structure for completed transaction information
pub struct CompletedTransaction {
    pub thread_id: usize,
    pub transaction: SanitizedTransaction,
}

/// PrioGraphScheduler is responsible for efficiently scheduling a batch of transactions
/// across multiple worker threads while maintaining transaction dependencies and avoiding conflicts.
/// It also handles the completion of transactions and unlocking of accounts.
pub struct PrioGraphScheduler {
    /// Senders for dispatching transactions to different worker threads
    senders: Vec<Sender<Vec<ScheduledTransaction>>>,
    /// Data structure for managing account lock states to prevent conflicts
    account_locks: ThreadAwareAccountLocks,
    /// Window size for processing transactions in batches
    window_size: usize,
    /// Generator for unique transaction IDs
    id_generator: SeqIdGenerator,
    /// Receiver for completed transaction information
    completed_receiver: Receiver<CompletedTransaction>,
}

impl PrioGraphScheduler {
    /// Creates a new PrioGraphScheduler
    ///
    /// # Arguments
    /// * `senders` - A vector of senders for dispatching transactions to worker threads
    /// * `completed_receiver` - A receiver for completed transaction information
    pub fn new(
        senders: Vec<Sender<Vec<ScheduledTransaction>>>,
        completed_receiver: Receiver<CompletedTransaction>,
    ) -> Self {
        let len = senders.len();
        Self {
            senders,
            account_locks: ThreadAwareAccountLocks::new(len),
            window_size: 64, // Default window size
            id_generator: SeqIdGenerator::default(),
            completed_receiver,
        }
    }

    /// Schedules a batch of transactions across available worker threads
    ///
    /// # Arguments
    /// * `transactions` - A vector of transactions to be scheduled
    ///
    /// This method processes the transactions in windows, updates the priority graph,
    /// and distributes them to worker threads while avoiding conflicts.
    /// Schedules a batch of transactions across available worker threads
    ///
    /// # Arguments
    /// * `transactions` - A vector of transactions to be scheduled
    ///
    /// This method processes the transactions in windows, updates the priority graph,
    /// and distributes them to worker threads while avoiding conflicts.
    pub fn schedule_batch(&mut self, transactions: Vec<SanitizedTransaction>) {
        let scheduled_transactions: Vec<ScheduledTransaction> = transactions
            .into_iter()
            .map(|tx| ScheduledTransaction {
                id: self.id_generator.gen().id(),
                priority: 0, // Default priority
                transaction: tx,
            })
            .collect();

        for window in scheduled_transactions.chunks(self.window_size) {
            let before_locks = self.account_locks.get_locked_address();
            println!("Total locks before processing window: {}", before_locks);
            self.process_window(window);
            let after_locks = self.account_locks.get_locked_address();
            println!("Total locks after processing window: {}", after_locks);
        }

        // Process completed transactions
        self.handle_completed_transactions();
    }

    /// Processes a window of transactions
    ///
    /// # Arguments
    /// * `window` - A slice of transactions to process in this window
    ///
    /// This method adds transactions to the priority graph, identifies non-conflicting
    /// batches, and sends them to appropriate worker threads.
    /// It also tracks the total number of locks acquired during processing.
    fn process_window(&mut self, window: &[ScheduledTransaction]) {
        let thread_num = self.senders.len();
        let mut schedule_batch = vec![vec![]; thread_num];
        for transaction in window {
            let message = transaction.transaction.message();
            let account_keys = message.account_keys();

            let write_account_locks = account_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| message.is_writable(index).then_some(key));
            let read_account_locks = account_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| (!message.is_writable(index)).then_some(key));

            if let Some(thread_id) = self.account_locks.try_lock_accounts(
                write_account_locks,
                read_account_locks,
                ThreadSet::any(thread_num),
                |thread_set| {
                    let mut rng = rand::thread_rng();
                    thread_set
                        .contained_threads_iter()
                        .choose(&mut rng)
                        .unwrap()
                },
            ) {
                schedule_batch[thread_id].push(transaction.clone());
            } else {
                // can't find a thread to schedule, currently just throw it.
            }
        }

        for (thread_id, batch) in schedule_batch.into_iter().enumerate() {
            self.send_batch_to_thread(thread_id, batch);
        }
    }

    /// Handles completed transactions by unlocking the corresponding accounts
    fn handle_completed_transactions(&mut self) {
        while let Ok(completed) = self.completed_receiver.try_recv() {
            let message = completed.transaction.message();
            let account_keys = message.account_keys();

            let write_account_locks = account_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| message.is_writable(index).then_some(key));
            let read_account_locks = account_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| (!message.is_writable(index)).then_some(key));

            self.account_locks.unlock_accounts(
                write_account_locks,
                read_account_locks,
                completed.thread_id,
            );

            let remaining_locks = self.account_locks.get_locked_address();
            println!(
                "Received data from thread {}, remaining locks after unlock: {}",
                completed.thread_id, remaining_locks
            );
        }
    }

    /// Sends a batch of transactions to the specified worker thread
    fn send_batch_to_thread(&mut self, thread_id: usize, batch: Vec<ScheduledTransaction>) {
        if let Err(e) = self.senders[thread_id].send(batch) {
            eprintln!(
                "Failed to send transaction to thread {}: {:?}",
                thread_id, e
            );
        }
    }
}
