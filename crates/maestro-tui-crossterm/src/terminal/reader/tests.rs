//! Subscription shutdown, one-reader delivery, paused input and stream completion.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::Reader;
use crate::terminal::producer::{self, Arrival, Record, Records};

/// A source that reports a pending record, then blocks as if another reader drained it.
struct Drained {
    /// Reports entry into the actual read.
    entered: Sender<()>,
    /// The record that eventually arrives.
    input: Receiver<Vec<Record>>,
}

impl Records for Drained {
    fn pending(&self) -> io::Result<u32> {
        Ok(1)
    }

    fn read(&self) -> io::Result<Vec<Record>> {
        self.entered.send(()).unwrap();
        self.input.recv().map_err(io::Error::other)
    }
}

/// A character-producing record.
fn typed(unit: u8) -> Vec<Record> {
    vec![Record::Key {
        down: true,
        key: 0,
        unit: u16::from(unit),
        repeat: 1,
        control: 0,
    }]
}

/// Receives one byte arrival.
fn bytes(arrivals: &mut UnboundedReceiver<Arrival>) -> Vec<u8> {
    match arrivals.blocking_recv().unwrap() {
        Arrival::Bytes(bytes) => bytes,
        other => panic!("expected bytes, got {other:?}"),
    }
}

#[test]
fn stopping_after_another_reader_drains_the_count_never_joins_the_read() {
    let (entered, began) = channel();
    let (input, read) = channel();
    let reader = Reader::spawn(producer::console(Drained {
        entered,
        input: read,
    }))
    .unwrap();
    let (sender, mut arrivals) = unbounded_channel();
    let subscription = reader.subscribe(sender);
    began.recv().unwrap();
    drop(subscription);
    assert!(arrivals.blocking_recv().is_none());
    input.send(typed(b'x')).unwrap();
}

#[test]
fn a_record_arriving_after_stop_is_held_and_delivered_once_on_restart() {
    let (entered, began) = channel();
    let (input, read) = channel();
    let reader = Reader::spawn(producer::console(Drained {
        entered,
        input: read,
    }))
    .unwrap();
    let (sender, mut first) = unbounded_channel();
    let subscription = reader.subscribe(sender);
    began.recv().unwrap();
    drop(subscription);
    assert!(first.blocking_recv().is_none());
    input.send(typed(b'x')).unwrap();
    while reader.0.lock().retained.is_empty() {
        std::thread::yield_now();
    }
    let (sender, mut resumed) = unbounded_channel();
    let subscription = reader.subscribe(sender);
    assert_eq!(bytes(&mut resumed), b"x");
    began.recv().unwrap();
    drop(subscription);
    assert!(resumed.blocking_recv().is_none());
    input.send(Vec::new()).unwrap();
}

/// An idle console that reports when the thread reached its pending query.
struct Idle {
    /// Its one entry notification.
    entered: Mutex<Option<Sender<()>>>,
    /// Counts reads, which must stay zero.
    reads: Arc<AtomicUsize>,
}

impl Records for Idle {
    fn pending(&self) -> io::Result<u32> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            entered.send(()).unwrap();
        }
        Ok(0)
    }

    fn read(&self) -> io::Result<Vec<Record>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
}

#[test]
fn stopping_with_no_input_pending_joins_without_reading() {
    let (entered, began) = channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let source = Idle {
        entered: Mutex::new(Some(entered)),
        reads: Arc::clone(&reads),
    };
    let reader = Reader::spawn(producer::console(source)).unwrap();
    let (sender, mut arrivals) = unbounded_channel();
    let subscription = reader.subscribe(sender);
    began.recv().unwrap();
    drop(subscription);
    assert!(arrivals.blocking_recv().is_none());
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn concurrent_subscribers_share_one_read_and_each_receive_the_input() {
    let (entered, began) = channel();
    let (input, read) = channel();
    let reader = Reader::spawn(producer::console(Drained {
        entered,
        input: read,
    }))
    .unwrap();
    let (sender, mut terminal) = unbounded_channel();
    let terminal_subscription = reader.subscribe(sender);
    began.recv().unwrap();
    let (sender, mut wait) = unbounded_channel();
    let wait_subscription = reader.subscribe(sender);
    input.send(typed(b'x')).unwrap();
    assert_eq!(bytes(&mut terminal), b"x");
    assert_eq!(bytes(&mut wait), b"x");
    began.recv().unwrap();
    drop(wait_subscription);
    assert!(wait.blocking_recv().is_none());
    input.send(typed(b'y')).unwrap();
    assert_eq!(bytes(&mut terminal), b"y");
    drop(terminal_subscription);
}

#[test]
fn eof_closes_current_and_later_subscriptions() {
    let reader = Reader::spawn(producer::pipe(io::Cursor::new(b"pipe"))).unwrap();
    let (sender, mut arrivals) = unbounded_channel();
    let _subscription = reader.subscribe(sender);
    assert_eq!(bytes(&mut arrivals), b"pipe");
    assert!(arrivals.blocking_recv().is_none());
    let (sender, mut later) = unbounded_channel();
    let _subscription = reader.subscribe(sender);
    assert!(later.blocking_recv().is_none());
}

#[test]
fn failures_reach_every_subscriber_and_end_the_source() {
    let (entered, began) = channel();
    let (release, released) = channel();
    let reader = Reader::spawn(move || {
        entered.send(()).unwrap();
        released.recv().unwrap();
        Err(io::ErrorKind::InvalidInput.into())
    })
    .unwrap();
    let (sender, mut first) = unbounded_channel();
    let _first = reader.subscribe(sender);
    began.recv().unwrap();
    let (sender, mut second) = unbounded_channel();
    let _second = reader.subscribe(sender);
    release.send(()).unwrap();
    for arrivals in [&mut first, &mut second] {
        assert!(
            matches!(arrivals.blocking_recv(), Some(Arrival::Failed(error)) if error.kind() == io::ErrorKind::InvalidInput)
        );
        assert!(arrivals.blocking_recv().is_none());
    }
}

#[test]
fn interrupted_reads_retry_and_a_closed_receiver_does_not_consume_the_batch() {
    let mut interrupted = true;
    let (entered, began) = channel();
    let (release, released) = channel();
    let reader = Reader::spawn(move || {
        if std::mem::take(&mut interrupted) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        // The worker reads again after the test returns; closed channels end its stream.
        if entered.send(()).is_err() || released.recv().is_err() {
            return Ok(None);
        }
        Ok(Some(vec![Arrival::Bytes(b"held".to_vec())]))
    })
    .unwrap();
    let (sender, arrivals) = unbounded_channel();
    let _subscription = reader.subscribe(sender);
    began.recv().unwrap();
    drop(arrivals);
    release.send(()).unwrap();
    while reader.0.lock().retained.is_empty() {
        std::thread::yield_now();
    }
    let (sender, mut arrivals) = unbounded_channel();
    let _subscription = reader.subscribe(sender);
    assert_eq!(bytes(&mut arrivals), b"held");
}

#[test]
fn forwarded_os_errors_keep_their_native_error_code() {
    let reader = Reader::spawn(|| Err(io::Error::from_raw_os_error(123))).unwrap();
    let (sender, mut arrivals) = unbounded_channel();
    let _subscription = reader.subscribe(sender);
    assert!(
        matches!(arrivals.blocking_recv(), Some(Arrival::Failed(error)) if error.raw_os_error() == Some(123))
    );
}
