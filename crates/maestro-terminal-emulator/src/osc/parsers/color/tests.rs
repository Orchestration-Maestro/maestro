//! Token consumption at the colour request owner.

use super::{Operation, ansi_set};

/// Count iterator calls, including the final exhausted read.
struct Counted<I> {
    /// Underlying monotone token iterator.
    inner: I,
    /// Calls to next, including exhaustion.
    calls: usize,
}
impl<I: Iterator> Iterator for Counted<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.calls += 1;
        self.inner.next()
    }
}

#[test]
fn ansi_pairs_visit_each_token_once() {
    for pairs in [64, 128] {
        let data = vec!["2;?"; pairs].join(";");
        let mut tokens = Counted {
            inner: data.as_bytes().split(|b| *b == b';'),
            calls: 0,
        };
        assert_eq!(ansi_set(Operation::Osc4, &mut tokens).len(), pairs);
        assert_eq!(tokens.calls, 2 * pairs + 1);
    }
}
