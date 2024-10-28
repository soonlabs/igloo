use ahash::{HashMap, HashMapExt};

/// Represents the current status of an SVM worker, including its duration.
#[derive(Debug, Clone)]
pub enum SvmWorkerSlicingStatus {
    /// Worker is actively processing transactions.
    Active { start: u64, end: u64 },
    /// Worker is idle, waiting for new transactions.
    Idle { start: u64, end: u64 },
}

impl SvmWorkerSlicingStatus {
    /// Period start time (Unix timestamp in milliseconds).
    pub fn start(&self) -> u64 {
        match self {
            SvmWorkerSlicingStatus::Active { start, .. }
            | SvmWorkerSlicingStatus::Idle { start, .. } => *start,
        }
    }

    /// Period end time (Unix timestamp in milliseconds).
    pub fn end(&self) -> u64 {
        match self {
            SvmWorkerSlicingStatus::Active { end, .. }
            | SvmWorkerSlicingStatus::Idle { end, .. } => *end,
        }
    }

    /// Returns true if the status is Active.
    pub fn is_active(&self) -> bool {
        matches!(self, SvmWorkerSlicingStatus::Active { .. })
    }

    /// Returns true if the status is Idle.
    pub fn is_idle(&self) -> bool {
        matches!(self, SvmWorkerSlicingStatus::Idle { .. })
    }
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
    summary.merged_windows = merged_windows.len() as u64;

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
///
/// This function takes a vector of time windows (start, end) and merges
/// overlapping or adjacent windows into single, continuous windows.
///
/// # Examples:
///
/// 1. Non-overlapping windows:
///    Input:  [(1, 3), (5, 7), (9, 11)]
///    Output: [(1, 3), (5, 7), (9, 11)]
///
/// 2. Overlapping windows:
///    Input:  [(1, 5), (2, 6), (3, 7)]
///    Output: [(1, 7)]
///
/// 3. Adjacent windows:
///    Input:  [(1, 3), (3, 5), (7, 9)]
///    Output: [(1, 5), (7, 9)]
///
/// 4. Mixed case:
///    Input:  [(1, 3), (2, 4), (5, 7), (6, 8), (10, 12)]
///    Output: [(1, 4), (5, 8), (10, 12)]
fn merge_time_windows(windows: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    if windows.is_empty() {
        return vec![];
    }

    let mut merged = vec![windows[0]];

    for window in windows.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if window.0 <= last.1 {
            // If the start of the current window is less than or equal to
            // the end of the last merged window, we have an overlap or adjacent windows.
            // Extend the last merged window to cover both.
            last.1 = last.1.max(window.1);
        } else {
            // If there's no overlap, add the current window as a new entry.
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
    /// total merged windows
    pub merged_windows: u64,
}

impl ThreadLoadSummary {
    /// Prints a human-readable summary of the thread load and coverage.
    pub fn print_summary(&self) {
        println!("Thread Load Summary:");
        println!("Total Merged Windows: {}", self.merged_windows);
        println!("Total Duration: {} ms", self.total_duration);
        println!("Average Load: {:.2}%", self.average_load * 100.0);
        println!("Overall Thread Coverage: {:.2}%", self.average_load * 100.0);
    }
}
