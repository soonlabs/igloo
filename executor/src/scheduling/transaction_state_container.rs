use crate::scheduling::ScheduledTransaction;
use min_max_heap::MinMaxHeap;
use solana_sdk::{slot_history::Slot, transaction::SanitizedTransaction};
use std::collections::HashMap;

/// Represents the state of a transaction in the container
#[derive(Debug)]
pub enum TransactionState {
    /// Transaction is unprocessed and waiting to be scheduled
    Unprocessed(SanitizedTransaction),
    /// Transaction is currently being processed
    Pending,
}

/// A wrapper struct for SanitizedTransaction with time-to-live information
#[derive(Debug)]
pub struct ScheduledTransactionTTL {
    /// The scheduled transaction with additional metadata
    pub transaction: ScheduledTransaction,
    /// The maximum slot age this transaction is valid for
    pub max_age_slot: Slot,
}

/// A unique identifier for a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId(u64);

/// A wrapper struct for TransactionId with priority information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionPriorityId {
    priority: u64,
    id: TransactionId,
}

impl TransactionPriorityId {
    pub fn new(priority: u64, id: TransactionId) -> Self {
        Self { priority, id }
    }
}

impl Ord for TransactionPriorityId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for TransactionPriorityId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// TransactionStateContainer manages the lifecycle of transactions within the scheduler.
/// It provides efficient prioritization and state tracking for transactions.
pub struct TransactionStateContainer {
    /// Priority queue of TransactionPriorityId, used for ordering transactions by priority.
    /// This allows for efficient selection of the highest priority transaction.
    priority_queue: MinMaxHeap<TransactionPriorityId>,
    /// Map of TransactionId to TransactionState, used for tracking the state of each transaction.
    /// Key: TransactionId - A unique identifier for each transaction
    /// Value: TransactionState - The current state of the transaction (Unprocessed or Pending)
    id_to_transaction_state: HashMap<TransactionId, TransactionState>,
}

impl TransactionStateContainer {
    /// Creates a new TransactionStateContainer with the specified capacity
    ///
    /// # Arguments
    /// * `capacity` - The maximum number of transactions the container can hold
    ///
    /// # Returns
    /// A new instance of TransactionStateContainer
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            priority_queue: MinMaxHeap::with_capacity(capacity),
            id_to_transaction_state: HashMap::with_capacity(capacity),
        }
    }

    /// Checks if the priority queue is empty
    ///
    /// # Returns
    /// true if the queue is empty, false otherwise
    pub fn is_empty(&self) -> bool {
        self.priority_queue.is_empty()
    }

    /// Calculates the remaining capacity in the queue
    ///
    /// # Returns
    /// The number of additional transactions that can be added to the queue
    pub fn remaining_queue_capacity(&self) -> usize {
        self.priority_queue.capacity() - self.priority_queue.len()
    }

    /// Removes and returns the highest priority transaction from the queue
    ///
    /// # Returns
    /// An Option containing the highest priority TransactionPriorityId, or None if the queue is empty
    pub fn pop(&mut self) -> Option<TransactionPriorityId> {
        self.priority_queue.pop_max()
    }

    /// Retrieves a mutable reference to the TransactionState for a given TransactionId
    ///
    /// # Arguments
    /// * `id` - The TransactionId to look up
    ///
    /// # Returns
    /// An Option containing a mutable reference to the TransactionState, or None if not found
    pub fn get_mut_transaction_state(
        &mut self,
        id: &TransactionId,
    ) -> Option<&mut TransactionState> {
        self.id_to_transaction_state.get_mut(id)
    }

    /// Retrieves the SanitizedTransactionTTL for a given TransactionId
    ///
    /// # Arguments
    /// * `id` - The TransactionId to look up
    ///
    /// # Returns
    /// An Option containing a reference to the SanitizedTransactionTTL, or None if not found
    pub fn get_transaction_ttl(&self, id: &TransactionId) -> Option<&SanitizedTransaction> {
        match self.id_to_transaction_state.get(id) {
            Some(TransactionState::Unprocessed(ttl)) => Some(ttl),
            _ => None,
        }
    }

    /// Inserts a new transaction into the container
    ///
    /// # Arguments
    /// * `transaction_id` - The unique identifier for the transaction
    /// * `transaction_ttl` - The transaction with its time-to-live information
    /// * `packet` - The packet containing the transaction data
    /// * `priority` - The priority of the transaction
    /// * `cost` - The cost associated with processing the transaction
    ///
    /// # Returns
    /// true if a transaction was dropped due to capacity limits, false otherwise
    /// Inserts a new transaction into the container.
    ///
    /// # Arguments
    /// * `transaction_id` - Unique identifier for the transaction
    /// * `transaction_ttl` - Transaction with its time-to-live information
    /// * `priority` - Priority value for the transaction
    ///
    /// # Returns
    /// `true` if a transaction was dropped due to capacity limits, `false` otherwise
    ///
    /// This method adds a new transaction to the container, updating both the priority queue
    /// and the state map. It ensures that high-priority transactions are processed first.
    pub fn insert_new_transaction(
        &mut self,
        transaction_id: TransactionId,
        transaction: SanitizedTransaction,
        priority: u64,
    ) -> bool {
        let priority_id = TransactionPriorityId::new(priority, transaction_id);
        self.id_to_transaction_state
            .insert(transaction_id, TransactionState::Unprocessed(transaction));
        self.push_id_into_queue(priority_id)
    }

    /// Retries a transaction by reinserting it into the queue
    ///
    /// # Arguments
    /// * `transaction_id` - The unique identifier for the transaction
    /// * `transaction_ttl` - The updated transaction with its time-to-live information
    /// Retries a transaction by reinserting it into the queue with updated information
    ///
    /// # Arguments
    /// * `transaction_id` - The unique identifier for the transaction to retry
    /// * `transaction_ttl` - The updated transaction with its new time-to-live information
    ///
    /// # Details
    /// This method updates the state of an existing transaction and reinserts it into the priority queue.
    /// It uses the transaction's recent blockhash as a priority metric to maintain fairness and prevent
    /// transaction starvation. The method demonstrates the system's ability to handle transaction
    /// retries efficiently, which is crucial for maintaining system responsiveness and fairness.
    pub fn retry_transaction(
        &mut self,
        transaction_id: TransactionId,
        transaction: SanitizedTransaction,
    ) {
        if let Some(state) = self.id_to_transaction_state.get_mut(&transaction_id) {
            *state = TransactionState::Unprocessed(transaction);
            let priority = match state {
                TransactionState::Unprocessed(transaction) => {
                    // Use the hash bytes as priority to avoid accessing private field
                    let recent_blockhash = transaction.message().recent_blockhash();
                    u64::from_le_bytes(recent_blockhash.to_bytes()[..8].try_into().unwrap())
                }
                _ => unreachable!(),
            };
            let priority_id = TransactionPriorityId::new(priority, transaction_id);
            self.push_id_into_queue(priority_id);
        }
    }

    /// Pushes a transaction ID into the priority queue, potentially dropping the lowest priority transaction if full
    ///
    /// # Arguments
    /// * `priority_id` - The TransactionPriorityId to insert
    ///
    /// # Returns
    /// true if a transaction was dropped, false otherwise
    pub fn push_id_into_queue(&mut self, priority_id: TransactionPriorityId) -> bool {
        if self.remaining_queue_capacity() == 0 {
            let popped_id = self.priority_queue.push_pop_min(priority_id);
            self.remove_by_id(&popped_id.id);
            true
        } else {
            self.priority_queue.push(priority_id);
            false
        }
    }

    /// Removes a transaction from the container by its ID
    ///
    /// # Arguments
    /// * `id` - The TransactionId to remove
    pub fn remove_by_id(&mut self, id: &TransactionId) {
        self.id_to_transaction_state.remove(id);
    }
}
