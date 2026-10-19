//! Retained tab destinations across storage sizes.

use super::Tabstops;

#[test]
fn tab_positions_basic() {
    let mut tabs = Tabstops::init(80, 0);
    assert!(!tabs.get(4));
    tabs.set(4);
    assert!(tabs.get(4));
    assert!(!tabs.get(3));
    tabs.reset(0);
    assert!(!tabs.get(4));
    tabs.set(4);
    assert!(tabs.get(4));
    tabs.unset(4);
    assert!(!tabs.get(4));
}

#[test]
fn tab_positions_dynamic_allocations() {
    let mut tabs = Tabstops::init(0, 0);
    tabs.resize(1024);
    tabs.set(517);
    assert!(tabs.get(517));
    assert!(!tabs.get(516));
    assert!(!tabs.get(5));
    assert_eq!((0..1024).filter(|&col| tabs.get(col)).count(), 1);
    tabs.resize(80);
    tabs.resize(1024);
    assert!(tabs.get(517));
    assert!(!tabs.get(516));
}

#[test]
fn tab_positions_interval() {
    let tabs = Tabstops::init(80, 4);
    assert!(!tabs.get(0));
    assert!(tabs.get(4));
    assert!(!tabs.get(5));
    assert!(tabs.get(8));
}

#[test]
fn tab_positions_interval_with_zero_columns() {
    let tabs = Tabstops::init(0, 8);
    assert_eq!(tabs.cols, 0);
}

#[test]
fn tab_positions_count_on_80() {
    let tabs = Tabstops::init(80, 8);
    assert_eq!((0..80).filter(|&col| tabs.get(col)).count(), 9);
    assert!(!Tabstops::init(9, 8).get(8));
}

#[test]
fn tabstop_clear_is_idempotent_at_storage_boundaries() {
    let mut tabs = Tabstops::init(1024, 0);
    for col in [3, 511, 512, 517] {
        tabs.set(col - 1);
        tabs.set(col + 1);
        tabs.unset(col);
        tabs.unset(col);
        assert!(!tabs.get(col));
        assert!(tabs.get(col - 1));
        assert!(tabs.get(col + 1));
        tabs.set(col);
        tabs.unset(col);
        tabs.unset(col);
        assert!(!tabs.get(col));
        assert!(tabs.get(col - 1));
        assert!(tabs.get(col + 1));
        tabs.reset(0);
    }
}
