//! Retained paragraph and underline admission at growing caller operands.
use crate::{MarkedOptions, rules::paragraph::Recognizer};

#[test]
fn paragraph_chunks_wait_for_later_setext_without_revisiting_lines() {
    let source = format!("{}===\n\nnext", "alpha\nbeta\n".repeat(64));
    let mut recognizer = Recognizer::new(MarkedOptions::Terminal);
    for end in 1..=source.len() {
        let _ = recognizer.extend(&source[..end]);
    }
    let result = recognizer.finish(&source);
    let heading = result.heading.unwrap();
    assert_eq!(
        &source[heading.content],
        "alpha\nbeta\n".repeat(64).trim_end_matches('\n')
    );
    assert_eq!(heading.level, 1);
    assert_eq!(&source[heading.consumed..], "next");
    assert_eq!(result.paragraph, Some("alpha\nbeta\n".repeat(64).len() + 3));
    let mut control = Recognizer::new(MarkedOptions::Terminal);
    let control = control.finish("alpha\nbeta\n\n===\n");
    assert!(control.heading.is_none());
    assert_eq!(control.paragraph, Some(10));
    let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
    assert!(matches!(
        recognizer.extend("ignored"),
        crate::rules::paragraph::Verdict::Stable(_)
    ));
    assert_eq!(super::WORK.with_borrow(|work| work.paragraph_bytes), work);
}

#[test]
fn long_paragraphs_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let source = ("alpha\nbeta\n".repeat(64) + "\n").repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, crate::Kind::Paragraph(_)))
                .count(),
            count
        );
        let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
        eprintln!("paragraphs={count}, source_bytes={bytes}, paragraph_bytes={work}");
        assert!(
            work <= 8 * bytes,
            "paragraph bytes {work} for {bytes} input bytes"
        );
    }
}

#[test]
fn adjacent_setext_headings_settle_before_later_heading_labels() {
    for count in [16, 32, 64] {
        let source = "alpha\nbeta\n===\n".repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        assert_eq!(document.nodes.len(), count);
        assert!(
            document
                .nodes
                .iter()
                .all(|node| matches!(node.kind, crate::Kind::Heading(1, _, _)))
        );
        let work = super::WORK.with_borrow(|work| work.paragraph_bytes);
        assert!(
            work <= 12 * bytes,
            "heading bytes {work} for {bytes} input bytes"
        );
    }
}
