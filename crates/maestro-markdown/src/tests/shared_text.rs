//! Shared authored values at the parser boundary.
use crate::{MappedText, parse};

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
fn mapped_text_repeated_reads_keep_storage_shared() {
    let source = "> é\n> 🦀";
    let document = parse(source.to_owned());
    let text = document.nodes[0].authored.clone();
    let chunks = text.chunks().map(str::as_ptr).collect::<Vec<_>>();
    let clone = text.clone();
    drop(document);
    for _ in 0..4 {
        assert_eq!(text.len(), source.len());
        assert_eq!(text.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(clone.chunks().map(str::as_ptr).collect::<Vec<_>>(), chunks);
        assert_eq!(text.to_owned_string(), source);
    }
}
