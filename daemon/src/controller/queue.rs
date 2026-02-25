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

#[cfg(test)]
mod tests {
    use super::SingleFlightQueue;
    use crate::error::AppError;
    use std::path::PathBuf;

    #[test]
    fn enqueue_and_start_on_empty_queue() {
        let mut queue = SingleFlightQueue::new(1);
        let first = PathBuf::from("/tmp/a.wav");
        queue.enqueue(first.clone()).expect("enqueue");
        assert_eq!(queue.start_next(), Some(first));
        assert_eq!(queue.start_next(), None);
    }

    #[test]
    fn queue_rejects_when_full() {
        let mut queue = SingleFlightQueue::new(1);
        queue.enqueue(PathBuf::from("/tmp/a.wav")).expect("enqueue");
        let error = queue
            .enqueue(PathBuf::from("/tmp/b.wav"))
            .expect_err("must be full");
        assert!(matches!(error, AppError::Controller(message) if message.contains("queue full")));
    }

    #[test]
    fn queue_fifo_ordering() {
        let mut queue = SingleFlightQueue::new(2);
        let a = PathBuf::from("/tmp/a.wav");
        let b = PathBuf::from("/tmp/b.wav");
        queue.enqueue(a.clone()).expect("enqueue a");
        queue.enqueue(b.clone()).expect("enqueue b");

        assert_eq!(queue.start_next(), Some(a));
        queue.mark_finished();
        assert_eq!(queue.start_next(), Some(b));
    }

    #[test]
    fn mark_finished_underflow_safe() {
        let mut queue = SingleFlightQueue::new(1);
        queue.mark_finished();
        queue.mark_finished();
        queue.enqueue(PathBuf::from("/tmp/a.wav")).expect("enqueue");
        assert!(queue.start_next().is_some());
    }
}
