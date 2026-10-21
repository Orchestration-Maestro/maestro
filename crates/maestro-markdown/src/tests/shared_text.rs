//! Shared authored values at the parser boundary.
use crate::{MappedText, parse};
use std::{collections::HashSet, mem::size_of_val, sync::Arc};

/// Unique live allocation payloads, excluding allocator headers and hash control bytes.
#[derive(Default)]
pub(crate) struct Retained {
    /// Allocation addresses already charged while the complete document stays live.
    seen: HashSet<usize>,
    /// Shared allocation payloads plus independently owned native capacities.
    pub(crate) bytes: usize,
    /// Unique backing string capacity, separate from view and native metadata.
    pub(crate) source_bytes: usize,
}

impl Retained {
    /// Charge shared string capacity without counting overlapping selected windows twice.
    pub(crate) fn text(&mut self, value: &Arc<String>) {
        if self.arc(value, value.capacity()) {
            self.source_bytes += value.capacity();
        }
    }

    /// Charge shared metadata and its backing capacity once, returning whether it is new.
    pub(crate) fn arc<T>(&mut self, value: &Arc<T>, capacity: usize) -> bool {
        if !self.seen.insert(Arc::as_ptr(value) as usize) {
            return false;
        }
        self.bytes += size_of_val(value.as_ref()) + capacity;
        true
    }
}

#[test]
fn mapped_text_preserves_owned_empty_and_unicode_values() {
    for value in [String::new(), "é\n🦀".to_owned()] {
        let text = MappedText::from(value.clone());
        assert_eq!(text.len(), value.len());
        assert_eq!(text.is_empty(), value.is_empty());
        assert_eq!(text.chunks().collect::<String>(), value);
        assert_eq!(text.to_owned_string(), value);
    }
    assert!(MappedText::default().is_empty());
    let source = "> é\n> > 🦀";
    let document = parse(source.to_owned());
    let authored = document.nodes[0].authored.clone();
    drop(document);
    assert_eq!(authored.len(), source.len());
    assert_eq!(authored.chunks().collect::<String>(), source);
    assert_eq!(authored.to_owned_string(), source);
}

#[test]
fn source_node_views_keep_shared_storage() {
    let source = "> é\n> 🦀";
    let owner = crate::source::ScopeSource::identity(source);
    let view = owner.view(0..source.len());
    let first = view.node(0..4, crate::Kind::Gap);
    let second = view.node(5..source.len(), crate::Kind::Gap);
    assert_eq!(
        first.authored.chunks().next().unwrap().as_ptr(),
        owner.text.as_ptr()
    );
    assert_eq!(
        second.authored.chunks().next().unwrap().as_ptr() as usize,
        owner.text.as_ptr() as usize + 5
    );
    let selected = view.select(std::iter::once(5..source.len()));
    assert_eq!(
        selected.text.as_ptr() as usize,
        owner.text.as_ptr() as usize + 5
    );
    drop(owner);
    assert_eq!(first.authored.to_owned_string(), "> é");
    assert_eq!(second.authored.to_owned_string(), "> 🦀");
}

#[test]
fn segmented_cursor_keeps_unmaterialized_source_and_atomic_provenance() {
    use crate::source::{Cursor, ScopeSource};
    let source = ScopeSource::identity("é\\|🦀");
    let transformed = source.pipes();
    let mut cursor = Cursor::default();
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    cursor.append(source.select(std::iter::once(0..2)));
    cursor.append(transformed.select(std::iter::once(2..7)));
    assert_eq!(cursor.len(), 7);
    let prefix = cursor.take(3).materialize();
    assert_eq!(&*prefix.text, "é|");
    assert_eq!(prefix.native(&(2..3)), Some(2..4));
    assert_eq!(prefix.local(&(3..4)), Some(2..3));
    let suffix = cursor.take(cursor.len());
    assert!(cursor.is_empty());
    assert_eq!(super::WORK.with_borrow(|work| work.source_bytes), 2);
    let tail = suffix.materialize();
    assert_eq!(&*tail.text, "🦀");
    assert_eq!(tail.text.as_ptr(), transformed.text[3..].as_ptr());
    assert_eq!(tail.extent(0..tail.text.len()), 4..8);
    assert!(cursor.take(0).materialize().text.is_empty());
}

#[test]
fn segmented_cursor_lines_cross_piece_boundaries_once() {
    use crate::source::{Cursor, ScopeSource};
    let source = ScopeSource::identity("é\n🦀\nlast");
    let mut cursor = Cursor::default();
    for range in [0..2, 2..3, 3..7, 7..10, 10..12] {
        cursor.append(source.select(std::iter::once(range)));
    }
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    for (text, native) in [("é\n", 0..3), ("🦀\n", 3..8), ("last", 8..12)] {
        let line = cursor.line().unwrap().materialize();
        assert_eq!(&*line.text, text);
        assert_eq!(line.extent(0..line.text.len()), native);
    }
    assert!(cursor.line().is_none());
    assert_eq!(cursor.len(), 0);
    assert_eq!(super::WORK.with_borrow(|work| work.framing_bytes), 12);
}

#[test]
fn mapped_text_repeated_reads_keep_storage_shared() {
    let source = "> é\n> 🦀";
    let document = parse(source.to_owned());
    let text = document.nodes[0].authored.clone();
    let chunks = text.chunks().map(str::as_ptr).collect::<Vec<_>>();
    let clone = text.clone();
    drop(document);
    let mut retained = Retained::default();
    text.retained(&mut retained);
    let before = retained.bytes;
    clone.retained(&mut retained);
    assert_eq!(retained.bytes, before);
    assert_eq!(retained.source_bytes, source.len());
    for _ in 0..4 {
        assert_eq!(text.len(), source.len());
        assert_eq!(text.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(clone.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(text.to_owned_string(), source);
        let mut after = Retained::default();
        text.retained(&mut after);
        clone.retained(&mut after);
        assert_eq!(after.bytes, before);
    }
}
