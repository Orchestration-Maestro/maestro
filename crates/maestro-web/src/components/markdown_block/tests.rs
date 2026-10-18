//! Escaped Markdown keeps authored markup inert.
use super::{
    admission::{escape_tags, visit},
    render::prepare,
};
use maestro_app::markdown::Kind;

/// Inputs whose escaping changes the structure around a native code span or block,
/// with the document HTML that shows every tag outside earlier stable code as text.
const SHIFTED: [(&str, &str); 7] = [
    (
        "<x y=\"`\"> `<img src=x onerror=alert(1)>`",
        "<p>&lt;x y=&quot;<code>&quot;&amp;gt; </code>&lt;img src=x onerror=alert(1)&gt;`</p>\n",
    ),
    (
        "<x y='`'> `<img src=x onerror=alert(1)>`",
        "<p>&lt;x y=&#39;<code>&#39;&amp;gt; </code>&lt;img src=x onerror=alert(1)&gt;`</p>\n",
    ),
    (
        "<pre>x</pre>\n    <img src=x onerror=alert(1)>",
        "<p>&lt;pre&gt;x&lt;/pre&gt;\n&lt;img src=x onerror=alert(1)&gt;</p>\n",
    ),
    (
        "<script>x</script>\n    <img src=x onerror=alert(1)>",
        "<p>&lt;script&gt;x&lt;/script&gt;\n&lt;img src=x onerror=alert(1)&gt;</p>\n",
    ),
    (
        "<style>x</style>\n    <img src=x onerror=alert(1)>",
        "<p>&lt;style&gt;x&lt;/style&gt;\n&lt;img src=x onerror=alert(1)&gt;</p>\n",
    ),
    (
        "<textarea>x</textarea>\n    <img src=x onerror=alert(1)>",
        "<p>&lt;textarea&gt;x&lt;/textarea&gt;\n&lt;img src=x onerror=alert(1)&gt;</p>\n",
    ),
    (
        "`<i>`\n\n<pre>x</pre>\n    <img src=x onerror=alert(1)>",
        "<p><code>&lt;i&gt;</code></p>\n<p>&lt;pre&gt;x&lt;/pre&gt;\n&lt;img src=x onerror=alert(1)&gt;</p>\n",
    ),
];

/// Tag starts that no complete tag shape covers, with the document HTML that shows them
/// as text.
const UNCLOSED: [(&str, &str); 3] = [
    (
        "<div onmouseover=alert(1)\n\nnext",
        "<p>&lt;div onmouseover=alert(1)</p>\n<p>next</p>\n",
    ),
    (
        "<p is a prime\n\nnext",
        "<p>&lt;p is a prime</p>\n<p>next</p>\n",
    ),
    (
        "a <div \"x\n<img src=x onerror=alert(1)//>-->",
        "<p>a &lt;div &quot;x\n&lt;img src=x onerror=alert(1)//&gt;--&gt;</p>\n",
    ),
];

#[test]
fn escaped_documents_open_no_element_in_raw_html() {
    for (source, _) in SHIFTED.into_iter().chain(UNCLOSED) {
        let document = escape_tags(source.to_owned());
        visit(&document.nodes, &mut |node| {
            if let Kind::Html(html) | Kind::HtmlBlock(html) = &node.kind {
                let opens = html
                    .as_bytes()
                    .windows(2)
                    .any(|pair| pair[0] == b'<' && pair[1].is_ascii_alphabetic());
                assert!(!opens, "{source:?} keeps raw {html:?}");
            }
        });
    }
}

#[test]
fn escaped_markup_stays_text_when_code_shifts() {
    for (source, html) in SHIFTED {
        assert_eq!(prepare(source, true).html, html, "{source:?}");
    }
}

#[test]
fn escaped_markup_stays_text_after_unclosed_tag_starts() {
    for (source, html) in UNCLOSED {
        assert_eq!(prepare(source, true).html, html, "{source:?}");
    }
}

#[test]
fn escaping_keeps_stable_code_and_autolinks_literal() {
    let cases = [
        (
            "<b>x</b> `<i>`",
            "<p>&lt;b&gt;x&lt;/b&gt; <code>&lt;i&gt;</code></p>\n",
        ),
        (
            "<https://example.com/a>",
            "<p><a target=\"_blank\" rel=\"noopener noreferrer\" \
             href=\"https://example.com/a\">https://example.com/a</a></p>\n",
        ),
        ("<!-- c -->\n\ntext", "<!-- c -->\n<p>text</p>\n"),
        ("a <b", "<p>a &lt;b</p>\n"),
    ];
    for (source, html) in cases {
        assert_eq!(prepare(source, true).html, html, "{source:?}");
    }
    let block = prepare("    <img src=x onerror=alert(1)>", true);
    assert_eq!(
        block.html,
        "<div class=\"maestro-code-slot\" data-maestro-listing=\"0\"></div>\n"
    );
    assert_eq!(block.code[0].code, "<img src=x onerror=alert(1)>\n");
    assert_eq!(prepare("<b>x</b>", false).html, "<p><b>x</b></p>\n");
}
