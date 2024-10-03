use std::time::{Duration, Instant};

pub struct StopWatch {
    start_time: Instant,
    clicks: Vec<(Duration, String)>,
}

impl StopWatch {
    pub fn new() -> Self {
        StopWatch {
            start_time: Instant::now(),
            clicks: Vec::new(),
        }
    }

    pub fn click(&mut self, note: impl Into<String>) {
        let elapsed = self.start_time.elapsed();
        self.clicks.push((elapsed, note.into()));
    }

    pub fn summary(&self) -> String {
        let mut result = String::new();
        result.push_str("StopWatch Summary:\n");

        if self.clicks.is_empty() {
            result.push_str("No clicks recorded.\n");
        } else {
            let mut last_time = Duration::from_secs(0);
            for (i, (time, note)) in self.clicks.iter().enumerate() {
                let duration = *time - last_time;
                result.push_str(&format!("[{}] - duration: {:?}\n", note, duration));
                last_time = *time;
            }
            result.push_str(&format!(
                "Total time: {:?}\n",
                self.clicks.last().unwrap().0
            ));
        }

        result
    }
}
