//! Counted observations of the actual stored-pattern prefix walk.
use super::model_patterns;
use maestro_app::SettingsListEntry;
use std::{cell::Cell, rc::Rc};

/// An iterator whose every request is observed by the test.
struct Counted<I> {
    /// The input iterator.
    inner: I,
    /// Requests made by the production walk.
    calls: Rc<Cell<usize>>,
}
impl<I: Iterator> Iterator for Counted<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.calls.set(self.calls.get() + 1);
        self.inner.next()
    }
}

#[test]
fn model_pattern_prefix_has_linear_work() {
    for size in [64, 128, 256] {
        let calls = Rc::new(Cell::new(0));
        let input = Counted {
            inner: (0..size).map(|i| SettingsListEntry::String(format!("model-{i}"))),
            calls: calls.clone(),
        };
        let (patterns, invalid) = model_patterns(input);
        assert!(!invalid);
        assert_eq!(
            patterns,
            (0..size).map(|i| format!("model-{i}")).collect::<Vec<_>>()
        );
        assert!(calls.get() <= size + 1);
        let calls = Rc::new(Cell::new(0));
        let input = Counted {
            inner: std::iter::once(SettingsListEntry::String("prefix".into()))
                .chain(std::iter::once(SettingsListEntry::Unknown(
                    serde_json::Value::Null,
                )))
                .chain((0..size).map(|i| SettingsListEntry::String(format!("suffix-{i}")))),
            calls: calls.clone(),
        };
        let (patterns, invalid) = model_patterns(input);
        assert!(invalid);
        assert_eq!(patterns, ["prefix"]);
        assert!(calls.get() <= 2);
        let rescans = Rc::new(Cell::new(0));
        for start in 0..size {
            let _ = Counted {
                inner: start..size,
                calls: Rc::clone(&rescans),
            }
            .count();
        }
        assert!(rescans.get() > size + 1, "rescan negative control");
    }
}
