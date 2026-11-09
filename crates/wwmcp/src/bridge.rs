//! The bridge of the tools that act on the window (open a file, close a tab, save it): a call
//! comes on a thread of the server, waits while the window answers it at its next frame, and
//! returns the answer. The window wakes for it through the waker it sets.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::tools::{Kind, Outcome, fail};

/// How long a call waits for the window.
pub const WAIT: Duration = Duration::from_secs(30);

type Waker = Box<dyn Fn() + Send + Sync>;

/// A request to the window: what to do, and where its answer goes.
pub struct Ask<Q> {
    pub what: Q,
    answer: Sender<Outcome>,
}

/// The bridge between the threads of the server and the window.
pub struct Bridge<Q> {
    to_window: Mutex<Sender<Ask<Q>>>,
    at_window: Mutex<Receiver<Ask<Q>>>,
    waker: Mutex<Option<Waker>>,
    /// The thread of the window, once it answered: a call from it would wait for itself.
    window: Mutex<Option<std::thread::ThreadId>>,
}

impl<Q: Send> Bridge<Q> {
    pub fn new() -> Arc<Self> {
        let (to_window, at_window) = channel();
        Arc::new(Self {
            to_window: Mutex::new(to_window),
            at_window: Mutex::new(at_window),
            waker: Mutex::new(None),
            window: Mutex::new(None),
        })
    }

    /// What wakes the window for a request (`egui::Context::request_repaint`).
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(waker));
    }

    /// Whether something wakes the window for a request.
    pub fn has_waker(&self) -> bool {
        self.waker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Asks the window and waits for its answer, at most `wait`.
    pub fn ask(&self, what: Q, wait: Duration) -> Outcome {
        let here = std::thread::current().id();
        if *self.window.lock().unwrap_or_else(|e| e.into_inner()) == Some(here) {
            return Err(fail(
                Kind::Failed,
                "a tool of the window was called from the window itself; call it from another thread",
            ));
        }
        let (answer, answered) = channel();
        let sent = self
            .to_window
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(Ask { what, answer });
        if sent.is_err() {
            return Err(fail(Kind::Failed, "the window is closed"));
        }
        if let Some(wake) = self
            .waker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            wake();
        }
        answered
            .recv_timeout(wait)
            .unwrap_or_else(|_| Err(fail(Kind::Failed, "the window did not answer in time")))
    }

    /// Answers every request waiting, at a frame of the window.
    pub fn answer(&self, mut run: impl FnMut(Q) -> Outcome) {
        *self.window.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current().id());
        let waiting: Vec<Ask<Q>> = {
            let at = self.at_window.lock().unwrap_or_else(|e| e.into_inner());
            at.try_iter().collect()
        };
        for ask in waiting {
            let _ = ask.answer.send(run(ask.what));
        }
    }
}

/// An answer of a tool of the window that names what it did.
pub fn done(what: impl Into<String>) -> Outcome {
    Ok(Value::from(what.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_asks_and_the_window_answers() {
        let bridge: Arc<Bridge<u32>> = Bridge::new();
        let woken = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = woken.clone();
        bridge.set_waker(move || flag.store(true, std::sync::atomic::Ordering::SeqCst));
        let asking = bridge.clone();
        let thread = std::thread::spawn(move || asking.ask(20, WAIT));
        let mut answered = false;
        for _ in 0..500 {
            bridge.answer(|n| {
                answered = true;
                Ok(Value::from(n + 1))
            });
            if answered {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(thread.join().unwrap().unwrap(), Value::from(21));
        assert!(woken.load(std::sync::atomic::Ordering::SeqCst));
        // The window does not wait for itself.
        assert!(bridge.ask(1, Duration::from_millis(10)).is_err());
        let late: Arc<Bridge<u32>> = Bridge::new();
        assert!(late.ask(1, Duration::from_millis(10)).is_err());
    }
}
