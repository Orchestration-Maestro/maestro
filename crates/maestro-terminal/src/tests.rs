//! Thread-local observations of terminal traversal work.

use std::{cell::RefCell, collections::HashMap};

#[path = "../tests/support/kitty_terminal.rs"]
pub(crate) mod kitty_terminal;

/// Work observed by the unit-test program at the operation that performs it.
#[derive(Debug, Default)]
pub(crate) struct Work {
    /// Cell visits during row movement, alignment and suffix-flag refreshes.
    pub(crate) row_shift: usize,
    /// Attempts to borrow pages.
    pub(crate) page_visits: usize,
    /// Input bytes searched or delivered to the parser.
    pub(crate) input_visits: usize,
    /// Indexed reads, including failed reads, keyed by the existing arena owner.
    pub(crate) queries: HashMap<u64, usize>,
}

thread_local! {
    /// Observations isolated to the thread running one test.
    pub(crate) static WORK: RefCell<Work> = RefCell::default();
}

#[test]
fn work_observations_are_thread_local() {
    use crate::{Terminal, stream::Stream, stream_terminal::Handler};

    let mut terminal = Terminal::new(8, 1, 0).unwrap();
    let mut stream = Stream::new(false);
    WORK.with_borrow_mut(|work| *work = Work::default());
    stream.next_slice(b"parent", &mut Handler::new(&mut terminal));
    let parent = WORK.with_borrow(|work| work.input_visits);
    assert_eq!(parent, 6);
    std::thread::spawn(|| {
        assert_eq!(WORK.with_borrow(|work| work.input_visits), 0);
        let mut terminal = Terminal::new(8, 1, 0).unwrap();
        Stream::new(false).next_slice(b"child", &mut Handler::new(&mut terminal));
        assert_eq!(WORK.with_borrow(|work| work.input_visits), 5);
    })
    .join()
    .unwrap();
    assert_eq!(WORK.with_borrow(|work| work.input_visits), parent);
    WORK.with_borrow_mut(|work| *work = Work::default());
    stream.next_slice(b"x", &mut Handler::new(&mut terminal));
    assert_eq!(WORK.with_borrow(|work| work.input_visits), 1);
}
