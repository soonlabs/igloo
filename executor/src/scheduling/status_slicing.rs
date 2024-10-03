/// Represents the current status of an SVM worker, including the duration of that status.
#[derive(Debug, Clone)]
pub enum SvmWorkerSlicingStatus {
    /// Worker is actively processing a batch of transactions.
    Active {
        /// Start time of the active period (Unix timestamp in milliseconds).
        start: u64,
        /// End time of the active period (Unix timestamp in milliseconds).
        end: u64,
    },
    /// Worker is idle, waiting for new transactions to process.
    Idle {
        /// Start time of the idle period (Unix timestamp in milliseconds).
        start: u64,
        /// End time of the idle period (Unix timestamp in milliseconds).
        end: u64,
    },
}

impl SvmWorkerSlicingStatus {
    /// Creates a new Active status with the given start and end times.
    pub fn new_active(start: u64, end: u64) -> Self {
        SvmWorkerSlicingStatus::Active { start, end }
    }

    /// Creates a new Idle status with the given start and end times.
    pub fn new_idle(start: u64, end: u64) -> Self {
        SvmWorkerSlicingStatus::Idle { start, end }
    }

    /// Returns the duration of the status in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        let (start, end) = match self {
            SvmWorkerSlicingStatus::Active { start, end } => (start, end),
            SvmWorkerSlicingStatus::Idle { start, end } => (start, end),
        };
        end.saturating_sub(*start)
    }
}

/// Represents a status update from a worker thread.
#[derive(Debug)]
pub struct WorkerStatusUpdate {
    /// The ID of the worker thread.
    pub thread_id: usize,
    /// The current status of the worker.
    pub status: SvmWorkerSlicingStatus,
}
