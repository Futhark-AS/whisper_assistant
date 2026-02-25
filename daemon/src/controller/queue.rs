use std::collections::VecDeque;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

#[derive(Debug)]
pub struct SingleFlightQueue {
    max_in_flight: usize,
    in_flight: usize,
    pending: VecDeque<PathBuf>,
}

impl SingleFlightQueue {
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight,
            in_flight: 0,
            pending: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, path: PathBuf) -> AppResult<()> {
        if self.in_flight + self.pending.len() >= self.max_in_flight {
            return Err(AppError::Controller(
                "single-flight queue full (max_in_flight=1)".to_owned(),
            ));
        }
        self.pending.push_back(path);
        Ok(())
    }

    pub fn start_next(&mut self) -> Option<PathBuf> {
        if self.in_flight >= self.max_in_flight {
            return None;
        }

        let next = self.pending.pop_front();
        if next.is_some() {
            self.in_flight += 1;
        }
        next
    }

    pub fn mark_finished(&mut self) {
        if self.in_flight > 0 {
            self.in_flight -= 1;
        }
    }
}
