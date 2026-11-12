//! The bridge of the tools that act on the window (open a file, close a tab, save it): a call
//! comes on a thread of the server, waits while the window answers it at its next frame, and
//! returns the answer. The window wakes for it through the waker it sets. A call that gave up
//! waiting is never run later: the caller was told it failed, so the window skips it.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::tools::{Kind, Outcome, fail};

/// How long a call waits for the window.
pub const WAIT: Duration = Duration::from_secs(30);

type Waker = Box<dyn Fn() + Send + Sync>;

/// The states of a request: it waits for the window, the window took it, or its asker gave up.
/// Only one of the two changes from `WAITING` happens, so a request is either run or given up.
const WAITING: u8 = 0;
const TAKEN: u8 = 1;
const GIVEN_UP: u8 = 2;

/// A request to the window: what to do, and where its answer goes.
pub struct Ask<Q> {
    pub what: Q,
    answer: Sender<Outcome>,
    /// Shared with the asker: `WAITING`, `TAKEN` or `GIVEN_UP`.
    state: Arc<AtomicU8>,
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

    /// Asks the window and waits for its answer, at most `wait`. A request the window did not
    /// take in that time is given up and never runs; one the window is already working on has
    /// `wait` once more to finish, and then fails with a text that says it may still finish.
    pub fn ask(&self, what: Q, wait: Duration) -> Outcome {
        let here = std::thread::current().id();
        if *self.window.lock().unwrap_or_else(|e| e.into_inner()) == Some(here) {
            return Err(fail(
                Kind::Failed,
                "a tool of the window was called from the window itself; call it from another thread",
            ));
        }
        let (answer, answered) = channel();
        let state = Arc::new(AtomicU8::new(WAITING));
        let sent = self
            .to_window
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(Ask {
                what,
                answer,
                state: state.clone(),
            });
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
        if let Ok(outcome) = answered.recv_timeout(wait) {
            return outcome;
        }
        // The time is over. A request still waiting is given up here, and the window skips it;
        // one the window took in the meantime runs, so its answer is waited for once more.
        let given_up = state
            .compare_exchange(WAITING, GIVEN_UP, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if given_up {
            return Err(fail(Kind::Failed, "the window did not answer in time"));
        }
        answered.recv_timeout(wait).unwrap_or_else(|_| {
            Err(fail(
                Kind::Failed,
                "the window began the request and did not finish in time; it may still finish",
            ))
        })
    }

    /// Answers every request waiting, at a frame of the window; a request whose asker gave up
    /// (its time was over) is dropped without running.
    pub fn answer(&self, mut run: impl FnMut(Q) -> Outcome) {
        *self.window.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current().id());
        let waiting: Vec<Ask<Q>> = {
            let at = self.at_window.lock().unwrap_or_else(|e| e.into_inner());
            at.try_iter().collect()
        };
        for ask in waiting {
            let taken = ask
                .state
                .compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok();
            if taken {
                let _ = ask.answer.send(run(ask.what));
            }
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

    #[test]
    fn a_request_given_up_never_runs() {
        let bridge: Arc<Bridge<u32>> = Bridge::new();
        // The window is busy (a blocking dialog): the asker gives up.
        let asking = bridge.clone();
        let failed = std::thread::spawn(move || asking.ask(7, Duration::from_millis(20)))
            .join()
            .unwrap()
            .unwrap_err();
        assert!(
            failed.message.contains("did not answer in time"),
            "{failed}"
        );
        // The window comes back: the request reported as failed is not run, a new one is.
        let asking = bridge.clone();
        let thread = std::thread::spawn(move || asking.ask(8, WAIT));
        let mut ran = Vec::new();
        for _ in 0..500 {
            bridge.answer(|n| {
                ran.push(n);
                Ok(Value::from(n))
            });
            if ran.contains(&8) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(thread.join().unwrap().unwrap(), Value::from(8));
        assert_eq!(ran, [8], "the request given up did not run");
    }

    #[test]
    fn a_request_the_window_took_is_waited_for() {
        let bridge: Arc<Bridge<u32>> = Bridge::new();
        let asking = bridge.clone();
        let thread = std::thread::spawn(move || asking.ask(1, Duration::from_millis(100)));
        // The window takes the request before its time is over and answers after it: the
        // request ran, so the asker gets its answer and no failure.
        let mut ran = false;
        for _ in 0..500 {
            bridge.answer(|n| {
                ran = true;
                std::thread::sleep(Duration::from_millis(150));
                Ok(Value::from(n + 1))
            });
            if ran {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ran);
        assert_eq!(thread.join().unwrap().unwrap(), Value::from(2));
    }
}
