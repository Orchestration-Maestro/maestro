//! Tests for the scoped worker threads.

use super::map;

#[test]
fn results_follow_item_order_whichever_thread_finishes_first() {
    let items: Vec<u64> = (0..256).collect();
    // Early items spin longest, so later items finish first on the other threads. The skew only
    // makes out-of-order completion likely; on one core the items finish in order regardless.
    let results = map(&items, |item| {
        (0..(256 - item) * 2_000).fold(*item, |value, _| std::hint::black_box(value))
    })
    .unwrap();
    assert_eq!(results, items);
}

#[test]
fn two_panicking_items_return_one_error_rather_than_a_scope_panic() {
    let workers = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    assert!(workers >= 2, "the two failing items need two workers");

    // Both items panic together, so the scope holds two panicked workers when it joins.
    let both_claimed = std::sync::Barrier::new(2);
    let result: Result<Vec<()>, String> = map(&[0, 1], |_| {
        both_claimed.wait();
        panic!("the failing item");
    });

    assert_eq!(result, Err("a worker thread panicked".to_owned()));
}

#[test]
fn a_panicking_item_fails_the_whole_map() {
    let result = map(&[1, 2, 3], |item| {
        assert_ne!(*item, 2, "the failing item");
        *item
    });
    assert_eq!(result, Err("a worker thread panicked".to_owned()));
}
