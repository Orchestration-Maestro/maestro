//! Shared keyed ownership and live traversal.
use maestro_extensions::footer_data_provider::live::Live;
use std::{cell::Cell, rc::Rc};

/// A last-owner release that edits the collection being released from.
struct Released(Rc<dyn Fn()>);
impl Drop for Released {
    fn drop(&mut self) {
        (self.0)();
    }
}

#[test]
fn live_widget_map_returns_removed_ownership_outside_guards() {
    let map = Live::<String, Rc<Released>>::new();
    let alias = map.clone();
    let releases = Rc::new(Cell::new(0));
    let count = releases.clone();
    let value = Rc::new(Released(Rc::new(move || {
        assert_eq!(alias.len(), 1);
        count.set(count.get() + 1);
    })));
    map.insert("a".into(), value);
    map.insert("a".into(), Rc::new(Released(Rc::new(|| {}))));
    assert_eq!(releases.get(), 1);
    map.clear();
    clone_reentry();
    edited_walk();
}

/// A value whose clone can edit the source collection.
struct ReentrantClone(Rc<dyn Fn()>);
impl Clone for ReentrantClone {
    fn clone(&self) -> Self {
        (self.0)();
        Self(self.0.clone())
    }
}

/// Cloning values through lookup and traversal permits reentrant edits.
fn clone_reentry() {
    let map = Live::<String, ReentrantClone>::new();
    let alias = map.clone();
    map.insert(
        "a".into(),
        ReentrantClone(Rc::new(move || {
            alias.insert("b".into(), ReentrantClone(Rc::new(|| {})));
        })),
    );
    assert!(map.get("a").is_some());
    assert_eq!(
        map.iter().map(|(key, _)| key).collect::<Vec<_>>(),
        ["a", "b"]
    );
    map.clear();
}

/// Removal, clearing and exhaustion keep the live alias and release boundaries.
fn edited_walk() {
    let map = Live::<String, Rc<Released>>::new();
    let alias = map.clone();
    let noop = || Rc::new(Released(Rc::new(|| {})));
    map.insert("a".into(), noop());
    map.insert("b".into(), noop());
    let mut walk = map.iter();
    assert_eq!(walk.next().unwrap().0, "a");
    let removed = map.remove("b").unwrap();
    drop(removed);
    map.insert("b".into(), noop());
    assert_eq!(walk.next().unwrap().0, "b");
    let observed = Rc::new(Cell::new(0));
    let count = observed.clone();
    map.insert(
        "c".into(),
        Rc::new(Released(Rc::new(move || {
            assert!(alias.is_empty());
            count.set(count.get() + 1);
            alias.insert("late".into(), Rc::new(Released(Rc::new(|| {}))));
        }))),
    );
    map.clear();
    assert_eq!(observed.get(), 1);
    assert_eq!(walk.next().unwrap().0, "late");
    assert!(walk.next().is_none());
    map.insert("after".into(), noop());
    assert!(walk.next().is_none());
    let alias = map.clone();
    map.insert(
        "remove".into(),
        Rc::new(Released(Rc::new(move || {
            assert!(!alias.contains_key("remove"));
            alias.insert("removed".into(), Rc::new(Released(Rc::new(|| {}))));
        }))),
    );
    drop(map.remove("remove"));
    assert!(map.contains_key("removed"));
    map.clear();
}

#[test]
fn live_widget_map_flat_storage_survives_large_edit_sequences() {
    let original = Live::<usize, usize>::new();
    let map = original.clone();
    drop(original);
    let mut expected = Vec::new();
    let mut seed = 1_u64;
    for _ in 0..100_000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let key = usize::try_from((seed >> 32) % 128).unwrap();
        match seed % 7 {
            0..=2 => {
                let old = expected
                    .iter()
                    .position(|(k, _)| *k == key)
                    .map(|i| expected[i].1);
                assert_eq!(map.replace(key, key + 1), old);
                if let Some(i) = expected.iter().position(|(k, _)| *k == key) {
                    expected[i].1 = key + 1;
                } else {
                    expected.push((key, key + 1));
                }
            }
            3 => {
                let _ = map.remove(&key);
                expected.retain(|(k, _)| *k != key);
            }
            4 => {
                map.clear();
                expected.clear();
            }
            5 => {
                let alias = map.clone();
                assert_eq!(alias.len(), expected.len());
            }
            _ => {
                assert_eq!(map.iter().collect::<Vec<_>>(), expected);
            }
        }
        assert_eq!(map.iter().collect::<Vec<_>>(), expected);
    }
    map.clear();
    for key in 0..100_000 {
        map.insert(key, key);
    }
    assert_eq!(map.len(), 100_000);
    drop(map);
}
