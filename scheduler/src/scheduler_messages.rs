use solana_program::clock::Slot;
use {solana_sdk::transaction::SanitizedTransaction, std::fmt::Display};

/// A unique identifier for a transaction batch.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TransactionBatchId(u64);

impl TransactionBatchId {
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Display for TransactionBatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TransactionBatchId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// A unique identifier for a transaction.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionId(u64);

impl TransactionId {
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TransactionId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Scheduling unit.
pub struct SchedulingBatch {
    pub batch_id: TransactionBatchId,
    pub ids: Vec<TransactionId>,
    pub transactions: Vec<SanitizedTransaction>,
    pub max_ages: Vec<MaxAge>,
}

impl SchedulingBatch {
    pub fn valid(&self) -> bool {
        self.transactions.len() == self.ids.len() && self.ids.len() == self.max_ages.len()
    }
}

/// The scheduling result from worker one time.
/// Since the `SchedulingBatch` will be dispute to different subset to multi workers,
/// the `SchedulingBatchResult` is not 1-1 with SchedulingBatch.
/// One `batch_id` may occur mostly `num_of_worker` times.
pub struct SchedulingBatchResult {
    // workload.
    pub batch: SchedulingBatch,
    // time slice status for this batch job.
    pub retryable_indexes: Vec<usize>,
}

/// A TTL flag for a transaction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct MaxAge {
    pub epoch_invalidation_slot: Slot,
    pub alt_invalidation_slot: Slot,
}
