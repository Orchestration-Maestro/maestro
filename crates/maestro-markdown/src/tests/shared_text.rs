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
