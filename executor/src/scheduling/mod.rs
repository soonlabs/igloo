use solana_sdk::transaction::SanitizedTransaction;

pub mod prio_graph_scheduler;
pub mod read_write_account_set;
pub mod seq_id_generator;
pub mod thread_aware_account_locks;
pub mod transaction_state_container;
pub mod stopwatch;
pub mod status_slicing;

/// Represents a scheduled transaction with additional metadata
///
/// This struct encapsulates a SanitizedTransaction along with a unique ID and priority,
/// allowing for more efficient tracking and prioritization in the scheduling process.
#[derive(Debug, Clone)]
pub struct ScheduledTransaction {
    /// The unique identifier for this transaction
    pub id: u64,
    /// The priority of this transaction in the scheduling queue
    pub priority: u64,
    /// The actual transaction data
    pub transaction: SanitizedTransaction,
}
