//! Code block progress through complete document admission.
#[test]
fn adjacent_fences_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let source =
            ("```rust\n".to_owned() + &"let x = 1;\n".repeat(16) + "```\n\n").repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        let blocks: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                crate::Kind::CodeBlock(body, Some(info)) => Some((body, info)),
                _ => None,
            })
            .collect();
        assert_eq!(blocks.len(), count);
        assert!(
            blocks
                .iter()
                .all(|(body, info)| **body == "let x = 1;\n".repeat(16) && *info == "rust")
        );
        let work = super::WORK.with_borrow(|work| work.fence_bytes);
        eprintln!("fences={count}, source_bytes={bytes}, fence_bytes={work}");
        assert!(
            work <= 4 * bytes,
            "fence bytes {work} for {bytes} input bytes"
        );
    }
}

#[test]
fn adjacent_indented_blocks_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let source = ("    x = 1;\n".repeat(16) + "\n# next\n\n").repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        let blocks: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                crate::Kind::CodeBlock(body, None) => Some(body),
                _ => None,
            })
            .collect();
        assert_eq!(blocks.len(), count);
        assert!(blocks.iter().all(|body| **body == "x = 1;\n".repeat(16)));
        let work = super::WORK.with_borrow(|work| work.code_bytes);
        eprintln!("indented={count}, source_bytes={bytes}, code_bytes={work}");
        assert!(
            work <= 6 * bytes,
            "code bytes {work} for {bytes} input bytes"
        );
    }
}

#[test]
fn fence_chunks_keep_capture_absence_and_closing_boundaries() {
    use crate::rules::fence::{Recognizer, Verdict};
    for (source, body, remaining) in [
        ("  ```rust\né\n  ```~`  \nnext", Some("é"), "\nnext"),
        ("```\n```", None, ""),
        ("```\n\n```", Some(""), ""),
        ("~~~info`\n😀\n", Some("😀"), ""),
        ("```", None, ""),
        ("````\n```\n", Some("```"), ""),
    ] {
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let mut recognizer = Recognizer::default();
        for (start, scalar) in source.char_indices() {
            let _ = recognizer.extend(&source[..start + scalar.len_utf8()]);
        }
        let result = recognizer.finish(source).unwrap();
        assert_eq!(result.body.map(|range| &source[range]), body);
        assert_eq!(&source[result.consumed..], remaining);
        assert!(super::WORK.with_borrow(|work| work.framing_bytes) <= source.len());
        let work = super::WORK.with_borrow(|work| work.fence_bytes);
        assert!(matches!(
            recognizer.extend("ignored"),
            Verdict::Stable(Some(_))
        ));
        assert_eq!(super::WORK.with_borrow(|work| work.fence_bytes), work);
    }
    assert!(Recognizer::default().finish("```bad`\nx\n```").is_none());
}

#[test]
fn indented_chunks_keep_blank_separators_and_final_spaces() {
    use crate::rules::code::{Recognizer, Verdict};
    for (source, consumed) in [
        (
            "    é\n \t\n   \t😀\n\nnext",
            Some("    é\n \t\n   \t😀\n\n".len()),
        ),
        ("    é\n \t", Some("    é\n \t".len())),
        ("    \n", None),
        ("     \n", Some(6)),
        ("   x\n", None),
        ("\tx", Some(2)),
    ] {
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let mut recognizer = Recognizer::default();
        for (start, scalar) in source.char_indices() {
            let _ = recognizer.extend(&source[..start + scalar.len_utf8()]);
        }
        assert_eq!(recognizer.finish(source), consumed);
        assert!(super::WORK.with_borrow(|work| work.framing_bytes) <= source.len());
        let work = super::WORK.with_borrow(|work| work.code_bytes);
        assert!(matches!(recognizer.extend("ignored"), Verdict::Stable(_)));
        assert_eq!(super::WORK.with_borrow(|work| work.code_bytes), work);
    }
}
