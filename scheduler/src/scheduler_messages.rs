use crate::status_slicing::SvmWorkerSlicingStatus;
use {
    solana_sdk::transaction::SanitizedTransaction,
    std::fmt::Display,
};

/// A unique identifier for a transaction batch.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TransactionBatchId(u64);

impl TransactionBatchId {
    pub fn new(index: u64) -> Self {
        Self(index)
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
