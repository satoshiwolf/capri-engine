use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::config::LEDGE_SCAN_INTERVAL_MS;
use crate::desktop::ledges::{Ledge, scan_ledges};
use crate::physics::collision::LedgeSurface;

pub struct LedgeScanner {
    ledges: Receiver<Vec<Ledge>>,
    cancel: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl LedgeScanner {
    pub fn new(skip_hwnd: isize) -> Self {
        Self::spawn(Duration::from_millis(LEDGE_SCAN_INTERVAL_MS), move || {
            scan_ledges(&[skip_hwnd])
        })
    }

    pub fn spawn<F>(interval: Duration, mut scan: F) -> Self
    where
        F: FnMut() -> Vec<Ledge> + Send + 'static,
    {
        let (ledge_tx, ledges) = mpsc::sync_channel(2);
        let (cancel, cancel_rx) = mpsc::channel();
        let worker =
            thread::spawn(move || Self::run_worker(interval, &mut scan, ledge_tx, cancel_rx));
        Self {
            ledges,
            cancel,
            worker: Some(worker),
        }
    }

    fn run_worker<F>(
        interval: Duration,
        scan: &mut F,
        ledge_tx: SyncSender<Vec<Ledge>>,
        cancel_rx: Receiver<()>,
    ) where
        F: FnMut() -> Vec<Ledge>,
    {
        loop {
            if matches!(
                ledge_tx.try_send(scan()),
                Err(mpsc::TrySendError::Disconnected(_))
            ) {
                break;
            }
            match cancel_rx.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    pub fn try_recv(&self) -> Result<Vec<Ledge>, mpsc::TryRecvError> {
        self.ledges.try_recv()
    }
}

impl Drop for LedgeScanner {
    fn drop(&mut self) {
        let _ = self.cancel.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn ledge_surfaces(ledges: &[Ledge]) -> Vec<LedgeSurface> {
    ledges
        .iter()
        .map(|ledge| LedgeSurface {
            hwnd: ledge.hwnd,
            left: ledge.left as f32,
            right: ledge.right as f32,
            y: ledge.y as f32,
            window_title: (!ledge.title.is_empty()).then(|| ledge.title.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn scanner_stops_and_joins_when_dropped() {
        let scans = Arc::new(AtomicUsize::new(0));
        let worker_scans = scans.clone();
        let scanner = LedgeScanner::spawn(Duration::from_millis(1), move || {
            worker_scans.fetch_add(1, Ordering::AcqRel);
            Vec::new()
        });

        thread::sleep(Duration::from_millis(20));
        assert!(scans.load(Ordering::Acquire) > 0);

        drop(scanner);
        let stopped_at = scans.load(Ordering::Acquire);
        thread::sleep(Duration::from_millis(20));
        assert_eq!(scans.load(Ordering::Acquire), stopped_at);
    }

    #[test]
    fn empty_window_titles_are_not_published() {
        let ledges = [Ledge {
            hwnd: 7,
            title: String::new(),
            left: 10,
            right: 20,
            y: 30,
            is_floor: false,
        }];

        assert_eq!(ledge_surfaces(&ledges)[0].window_title, None);
    }
}
