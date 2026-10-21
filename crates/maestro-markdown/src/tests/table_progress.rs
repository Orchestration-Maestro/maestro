//! Table grammar work through complete public document admission.
#[test]
fn adjacent_tables_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let source = ("a | b\n--- | ---\n".to_owned() + &"x | y\n".repeat(16) + "\n").repeat(count);
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        let tables: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                crate::Kind::Table(table) => Some(table),
                _ => None,
            })
            .collect();
        assert_eq!(tables.len(), count);
        assert!(tables.iter().all(|table| table.rows.len() == 16));
        let work = super::WORK.with_borrow(|work| work.table_bytes);
        eprintln!("tables={count}, source_bytes={bytes}, table_bytes={work}");
        assert!(
            work <= 6 * bytes,
            "table bytes {work} for {bytes} input bytes"
        );
    }
}

#[test]
fn table_chunks_keep_body_and_trailing_newlines_separate() {
    use crate::{
        MarkedOptions,
        rules::table::{Recognizer, Verdict},
    };
    let source = "é | b\n:-- | --:\nx | y\n\n\nnext";
    let mut recognizer = Recognizer::new(MarkedOptions::Terminal);
    for (start, scalar) in source.char_indices() {
        let _ = recognizer.extend(&source[..start + scalar.len_utf8()]);
    }
    let result = recognizer.finish(source).unwrap();
    assert_eq!(&source[result.header.clone()], "é | b");
    assert_eq!(&source[result.delimiter.clone()], ":-- | --:");
    assert_eq!(&source[result.body.clone().unwrap()], "x | y\n");
    assert_eq!(&source[result.consumed..], "next");
    let work = super::WORK.with_borrow(|work| work.table_bytes);
    let Verdict::Stable(Some(retained)) = recognizer.extend("ignored") else {
        panic!("settled table")
    };
    assert_eq!(retained, result);
    assert_eq!(super::WORK.with_borrow(|work| work.table_bytes), work);
    let mut absent_body = Recognizer::new(MarkedOptions::Terminal);
    let result = absent_body.finish("a | b\n--- | ---").unwrap();
    assert_eq!(result.body, None);
    let mut empty_body = Recognizer::new(MarkedOptions::Terminal);
    let result = empty_body.finish("a | b\n--- | ---\n").unwrap();
    assert_eq!(result.body, Some(result.consumed..result.consumed));
}

#[test]
fn table_row_interruptions_keep_caller_markup_policy() {
    use crate::{MarkedOptions, rules::table::Recognizer};
    for (suffix, ordinary, literal) in [
        ("<div>\nx | y\n", "", "<div>\nx | y\n"),
        ("<!--\nx | y\n", "", ""),
        ("```\nx | y\n", "", ""),
        ("```", "```", "```"),
        ("    x\n", "", ""),
        ("2. x\n", "2. x\n", "2. x\n"),
        ("1. x\n", "", ""),
    ] {
        let source = format!("a | b\n--- | ---\n{suffix}");
        for (options, expected) in [
            (MarkedOptions::Terminal, ordinary),
            (MarkedOptions::Browser { escape_html: true }, literal),
        ] {
            let mut recognizer = Recognizer::new(options);
            for (start, scalar) in source.char_indices() {
                let _ = recognizer.extend(&source[..start + scalar.len_utf8()]);
            }
            let result = recognizer.finish(&source).unwrap();
            assert_eq!(
                &source[result.body.unwrap()],
                expected,
                "{options:?} {suffix:?}"
            );
        }
    }
}
