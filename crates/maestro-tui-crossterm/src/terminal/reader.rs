//! One input reader with live subscriptions and retained input between starts.
//!
//! Reads run without the subscription lock. Removing a subscription never waits for a
//! console or pipe read, and a batch read with no subscriber is held for the next start.

use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use tokio::sync::mpsc::UnboundedSender;

use super::producer::Arrival;

/// The shared reader of one input source.
pub(super) struct Reader(Arc<Shared>);

/// A live recipient; dropping it stops forwarding before returning.
pub(super) struct Subscription {
    /// The reader's subscription state.
    shared: Arc<Shared>,
    /// Identifies this registration without keeping its channel open.
    identity: Arc<()>,
}

/// State shared by the reader thread and its subscribers.
#[derive(Default)]
struct Shared {
    /// The current recipients and input held while paused.
    state: Mutex<State>,
    /// Wakes the reader when input is wanted again.
    resumed: Condvar,
}

/// Input delivery state, never locked across a read or caller callback.
#[derive(Default)]
struct State {
    /// Live subscribers in registration order.
    subscribers: Vec<(Arc<()>, UnboundedSender<Arrival>)>,
    /// A completed read held while no subscriber was present.
    retained: Vec<Arrival>,
    /// Whether the source ended or the reader was released.
    ended: bool,
}

impl Shared {
    /// Locks the delivery state, recovering it if the reader panicked.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits until a subscriber wants input, or the reader is released.
    fn wanted(&self) -> bool {
        let state = self.resumed.wait_while(self.lock(), |state| {
            state.subscribers.is_empty() && !state.ended
        });
        !state
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .ended
    }
    /// Reads outside the delivery lock until EOF or release of the reader.
    fn run(&self, mut read: impl FnMut() -> io::Result<Option<Vec<Arrival>>>) {
        while self.wanted() {
            let result = read();
            if result
                .as_ref()
                .is_err_and(|error| error.kind() == io::ErrorKind::Interrupted)
            {
                continue;
            }
            let mut state = self.lock();
            if state.ended {
                break;
            }
            let ended = !matches!(&result, Ok(Some(_)));
            match result {
                Ok(Some(arrivals)) => state.deliver(arrivals),
                Err(error) => state.deliver(vec![Arrival::Failed(error)]),
                Ok(None) => {}
            }
            if ended {
                state.ended = true;
                state.subscribers.clear();
                break;
            }
        }
    }
}

impl Reader {
    /// Starts the sole thread reading this source. `None` is EOF; an empty batch is idle.
    ///
    /// # Errors
    /// Returns a failure to create the reader thread.
    pub(super) fn spawn(
        read: impl FnMut() -> io::Result<Option<Vec<Arrival>>> + Send + 'static,
    ) -> io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let reader = Arc::clone(&shared);
        thread::Builder::new()
            .name("terminal-input".into())
            .spawn(move || {
                reader.run(read);
            })?;
        Ok(Self(shared))
    }

    /// Subscribes to the source, first forwarding any batch held while it was paused.
    pub(super) fn subscribe(&self, sender: UnboundedSender<Arrival>) -> Subscription {
        let identity = Arc::new(());
        let mut state = self.0.lock();
        for arrival in state.retained.drain(..) {
            let _ = sender.send(arrival);
        }
        if !state.ended {
            state.subscribers.push((Arc::clone(&identity), sender));
        }
        self.0.resumed.notify_one();
        Subscription {
            shared: Arc::clone(&self.0),
            identity,
        }
    }
}

impl State {
    /// Delivers a batch to every live recipient, or holds it for the next start.
    fn deliver(&mut self, arrivals: Vec<Arrival>) {
        self.subscribers.retain(|(_, sender)| !sender.is_closed());
        if self.subscribers.is_empty() {
            self.retained.extend(arrivals);
            return;
        }
        for arrival in arrivals {
            for (_, sender) in &self.subscribers {
                let _ = sender.send(arrival.clone());
            }
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.shared
            .lock()
            .subscribers
            .retain(|(identity, _)| !Arc::ptr_eq(identity, &self.identity));
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.ended = true;
        state.subscribers.clear();
        self.0.resumed.notify_one();
    }
}

#[cfg(test)]
mod tests;
