//! The console reader thread: record translation, stopping and failures.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::{Arrival, Producer, Record, Records};

/// Records handed out one batch per read, with an optional gate each read waits on.
#[derive(Clone, Default)]
struct Script(Arc<Mutex<State>>);

/// What the scripted console holds and has done.
#[derive(Default)]
struct State {
    /// The batches still waiting, one per read.
    batches: VecDeque<Vec<Record>>,
    /// How many reads ran.
    reads: usize,
    /// The failure the next pending query returns.
    failure: Option<io::Error>,
    /// Told when a read begins, and waited on until the test releases it.
    gate: Option<(Sender<()>, Mutex<Receiver<()>>)>,
}

impl Script {
    /// A console whose reads hand out `batches` in order.
    fn new(batches: Vec<Vec<Record>>) -> Self {
        let script = Self::default();
        script.lock().batches = batches.into();
        script
    }

    /// The state, locked.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    /// How many reads ran.
    fn reads(&self) -> usize {
        self.lock().reads
    }
}

impl Records for Script {
    fn pending(&self) -> io::Result<u32> {
        let mut state = self.lock();
        if let Some(error) = state.failure.take() {
            return Err(error);
        }
        Ok(u32::try_from(state.batches.front().map_or(0, Vec::len)).unwrap())
    }

    fn read(&self) -> io::Result<Vec<Record>> {
        let gate = {
            let mut state = self.lock();
            state.reads += 1;
            state.gate.take()
        };
        if let Some((entered, release)) = gate {
            entered.send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
        }
        Ok(self.lock().batches.pop_front().unwrap_or_default())
    }
}

/// A key press producing `unit`.
fn press(unit: u16) -> Record {
    Record::Key {
        down: true,
        key: 0,
        unit,
        repeat: 1,
    }
}

/// The key presses of `text`, one per UTF-16 unit.
fn typed(text: &str) -> Vec<Record> {
    text.encode_utf16().map(press).collect()
}

/// Receives arrivals until the thread ends and its sender is gone.
fn all(mut arrivals: UnboundedReceiver<Arrival>) -> Vec<String> {
    let mut seen = Vec::new();
    while let Some(arrival) = arrivals.blocking_recv() {
        seen.push(match arrival {
            Arrival::Bytes(bytes) => String::from_utf8(bytes).unwrap(),
            Arrival::Resize => "<resize>".into(),
            Arrival::Failed(error) => format!("<failed {}>", error.kind()),
        });
    }
    seen
}

/// Receives the next `count` arrivals as text.
fn next(arrivals: &mut UnboundedReceiver<Arrival>, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|_| match arrivals.blocking_recv() {
            Some(Arrival::Bytes(bytes)) => bytes,
            other => panic!("expected bytes, got {other:?}"),
        })
        .collect()
}

#[test]
fn console_records_arrive_as_utf8_text_and_resizes_in_order() {
    let emoji: Vec<u16> = "😀".encode_utf16().collect();
    let mut first = typed("a\x1b[Z");
    first.extend([
        Record::Key {
            down: false,
            key: 0x41,
            unit: u16::from(b'a'),
            repeat: 1,
        },
        Record::Key {
            down: true,
            key: 0x10,
            unit: 0,
            repeat: 1,
        },
        Record::Other,
        press(emoji[0]),
    ]);
    let second = vec![
        press(emoji[1]),
        Record::Key {
            down: true,
            key: 0x42,
            unit: u16::from(b'b'),
            repeat: 3,
        },
        Record::Resize,
        press(0xdc00),
        Record::Key {
            down: false,
            key: super::VK_MENU,
            unit: u16::from(b'e'),
            repeat: 1,
        },
    ];
    let script = Script::new(vec![first, second]);
    let (sender, mut arrivals) = unbounded_channel();
    let producer = Producer::spawn(script, sender).unwrap();
    let received = next(&mut arrivals, 2);
    assert!(matches!(arrivals.blocking_recv(), Some(Arrival::Resize)));
    let last = next(&mut arrivals, 1);
    drop(producer);
    assert_eq!(received[0], b"a\x1b[Z");
    assert_eq!(received[1], "😀bbb".as_bytes());
    assert_eq!(last[0], "\u{fffd}e".as_bytes());
    assert_eq!(all(arrivals), Vec::<String>::new());
}

#[test]
fn stopping_with_no_input_pending_joins_without_reading() {
    let script = Script::default();
    let (sender, arrivals) = unbounded_channel();
    drop(Producer::spawn(script.clone(), sender).unwrap());
    assert_eq!(script.reads(), 0);
    assert_eq!(all(arrivals), Vec::<String>::new());
}

#[test]
fn stopping_during_a_read_waits_for_it_and_reads_nothing_after() {
    let script = Script::new(vec![typed("x"), typed("y")]);
    let (entered, began) = channel();
    let (release, released) = channel();
    script.lock().gate = Some((entered, Mutex::new(released)));
    let (sender, arrivals) = unbounded_channel();
    let producer = Producer::spawn(script.clone(), sender).unwrap();
    began.recv().unwrap();
    let flag = Arc::clone(&producer.stop);
    let stopping = std::thread::spawn(move || drop(producer));
    while !flag.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    stopping.join().unwrap();
    assert_eq!(script.reads(), 1);
    assert_eq!(script.lock().batches.len(), 1);
    assert_eq!(all(arrivals), ["x"]);
}

#[test]
fn stopping_after_a_completed_read_leaves_later_records_unread() {
    let script = Script::new(vec![typed("done")]);
    let (sender, mut arrivals) = unbounded_channel();
    let producer = Producer::spawn(script.clone(), sender).unwrap();
    assert_eq!(next(&mut arrivals, 1), [b"done".to_vec()]);
    drop(producer);
    script.lock().batches.push_back(typed("late"));
    assert_eq!(script.reads(), 1);
    assert_eq!(all(arrivals), Vec::<String>::new());
}

#[test]
fn a_console_failure_is_delivered_once_and_ends_the_thread() {
    let script = Script::default();
    script.lock().failure = Some(io::Error::from(io::ErrorKind::InvalidInput));
    let (sender, arrivals) = unbounded_channel();
    let producer = Producer::spawn(script.clone(), sender).unwrap();
    assert_eq!(all(arrivals), ["<failed invalid input parameter>"]);
    drop(producer);
    assert_eq!(script.reads(), 0);
}

#[test]
fn a_dropped_receiver_ends_the_thread_at_its_next_delivery() {
    let script = Script::new(vec![typed("a"), typed("b")]);
    let (sender, arrivals) = unbounded_channel();
    drop(arrivals);
    let mut producer = Producer::spawn(script.clone(), sender).unwrap();
    producer.thread.take().unwrap().join().unwrap();
    assert_eq!(script.reads(), 1);
}
