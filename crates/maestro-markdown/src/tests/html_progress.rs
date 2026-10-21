//! HTML block cost and append-only boundary controls.
#[test]
fn adjacent_html_blocks_advance_without_rescanning() {
    for options in [
        crate::MarkedOptions::Terminal,
        crate::MarkedOptions::Browser { escape_html: true },
    ] {
        for count in [16, 32, 64] {
            let block = "<!--\n".to_owned() + &"ordinary text\n".repeat(16) + "-->\n\n";
            let source = block.repeat(count);
            let bytes = source.len();
            super::WORK.with_borrow_mut(|work| *work = super::Work::default());
            let document = crate::parse_with_options(source, options);
            let blocks: Vec<_> = document
                .nodes
                .iter()
                .filter_map(|node| match &node.kind {
                    crate::Kind::HtmlBlock(body) => Some(body),
                    _ => None,
                })
                .collect();
            assert_eq!(blocks.len(), count);
            assert!(
                blocks
                    .iter()
                    .all(|body| **body == block.trim_end_matches('\n').to_owned() + "\n")
            );
            let work = super::WORK.with_borrow(|work| work.html_bytes);
            eprintln!("html={count}, options={options:?}, bytes={bytes}, work={work}");
            assert!(
                work <= 4 * bytes,
                "HTML bytes {work} for {bytes} input bytes"
            );
        }
    }
}

#[test]
fn html_chunks_preserve_family_terminators_and_blank_separators() {
    use crate::rules::html::{Recognizer, Verdict};
    for (source, raw) in [
        (
            "<script>é\n</SCRIPT> tail\n\nnext",
            "<script>é\n</SCRIPT> tail\n\n",
        ),
        ("<!--> tail\nnext", "<!--> tail\n"),
        ("<!--->\n\nnext", "<!--->\n\n"),
        ("<?é?>suffix", "<?é?>"),
        ("<!A>\n \nnext", "<!A>\n"),
        ("<![cdata[é]]>\n\nnext", "<![cdata[é]]>\n\n"),
        ("<div>\né\n \n\t\nnext", "<div>\né\n \n\t\n"),
        ("<x>\né\n\n \nnext", "<x>\né\n\n \n"),
        ("</x\n \n >\né\n\nnext", "</x\n \n >\né\n\n"),
        ("<pre>unterminated", "<pre>unterminated"),
    ] {
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let mut parser = Recognizer::default();
        for (start, scalar) in source.char_indices() {
            let _ = parser.extend(&source[..start + scalar.len_utf8()]);
        }
        assert_eq!(parser.finish(source), Some(raw.len()), "{source}");
        let work = super::WORK.with_borrow(|work| work.html_bytes);
        assert!(work <= source.len());
        assert!(matches!(parser.extend("ignored"), Verdict::Stable(Some(_))));
        assert_eq!(super::WORK.with_borrow(|work| work.html_bytes), work);
    }
    for source in [
        "</x\n >bad",
        "<scripture>\n",
        "    <!--",
        "<x a='unfinished\n>",
    ] {
        assert!(Recognizer::default().finish(source).is_none(), "{source}");
    }
}
