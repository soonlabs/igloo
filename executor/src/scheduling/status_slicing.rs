use ahash::{HashMap, HashMapExt};

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

/// Calculates and summarizes the thread load and coverage for a given set of worker status updates.
pub fn calculate_thread_load_summary(updates: &[WorkerStatusUpdate]) -> ThreadLoadSummary {
    let mut summary = ThreadLoadSummary::default();
    let mut time_windows: Vec<(u64, u64)> = Vec::new();
    let mut thread_statuses: HashMap<usize, Vec<SvmWorkerSlicingStatus>> = HashMap::new();

    // Collect all time windows and group statuses by thread
    for update in updates {
        let (start, end) = match update.status {
            SvmWorkerSlicingStatus::Active { start, end } => (start, end),
            SvmWorkerSlicingStatus::Idle { start, end } => (start, end),
        };
        time_windows.push((start, end));
        thread_statuses
            .entry(update.thread_id)
            .or_default()
            .push(update.status.clone());
    }

    // Sort and merge overlapping time windows
    time_windows.sort_by_key(|&(start, _)| start);
    let merged_windows = merge_time_windows(time_windows);

    let total_threads = thread_statuses.len() as f64;

    for (window_start, window_end) in merged_windows {
        let window_duration = window_end - window_start;
        let mut active_thread_time = 0;

        for statuses in thread_statuses.values() {
            for status in statuses {
                match status {
                    SvmWorkerSlicingStatus::Active { start, end } => {
                        if *start < window_end && *end > window_start {
                            let overlap_start = (*start).max(window_start);
                            let overlap_end = (*end).min(window_end);
                            active_thread_time += overlap_end - overlap_start;
                        }
                    }
                    SvmWorkerSlicingStatus::Idle { .. } => {}
                }
            }
        }

        let window_load = active_thread_time as f64 / (window_duration as f64 * total_threads);
        summary.total_duration += window_duration;
        summary.weighted_load += window_load * window_duration as f64;
    }

    summary.average_load = summary.weighted_load / summary.total_duration as f64;
    summary
}

/// Merges overlapping time windows.
fn merge_time_windows(windows: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    if windows.is_empty() {
        return vec![];
    }

    let mut merged = vec![windows[0]];

    for window in windows.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if window.0 <= last.1 {
            last.1 = last.1.max(window.1);
        } else {
            merged.push(window);
        }
    }

    merged
}

/// Represents a summary of thread load and coverage.
#[derive(Debug, Default)]
pub struct ThreadLoadSummary {
    /// The total duration of all merged time windows.
    pub total_duration: u64,
    /// The weighted sum of load across all time windows.
    pub weighted_load: f64,
    /// The average load across all time windows.
    pub average_load: f64,
}

impl ThreadLoadSummary {
    /// Prints a human-readable summary of the thread load and coverage.
    pub fn print_summary(&self) {
        println!("Thread Load Summary:");
        println!("Total Duration: {} ms", self.total_duration);
        println!("Average Load: {:.2}%", self.average_load * 100.0);
        println!("Overall Thread Coverage: {:.2}%", self.average_load * 100.0);
    }
}
