//! Append-only directive output through the shared escaping grammar.
use super::BlockRecognizer;

#[test]
fn directive_chunks_keep_escaped_tags_and_original_consumption() {
    for (source, raw, value) in [
        (
            "<!-- <x>é</x> -->\n\nnext",
            "<!-- <x>é</x> -->\n\n",
            "<!-- &lt;x&gt;é&lt;/x&gt; -->\n",
        ),
        (
            "<?<x?>tail>?>\nnext",
            "<?<x?>tail>?>\n",
            "<?&lt;x?&gt;tail>?>\n",
        ),
        ("<!--<x\né", "<!--<x\né", "<!--&lt;x\né"),
        ("<![cdata[é]]>suffix", "<![cdata[é]]>", "<![cdata[é]]>"),
    ] {
        let mut parser = BlockRecognizer::default();
        for (start, scalar) in source.char_indices() {
            let _ = parser.extend(&source[..start + scalar.len_utf8()]);
        }
        let result = parser.finish(source).unwrap();
        assert_eq!(result.consumed, raw.len(), "{source}");
        assert_eq!(result.value, value, "{source}");
        assert!(parser.extend("ignored"));
    }
    for source in ["<div>\n", "    <!--", "ordinary text"] {
        assert!(BlockRecognizer::default().finish(source).is_none());
    }
}
