//! Escaped Markdown keeps authored markup inert.
use super::render::prepare;
use maestro_app::markdown::{self, Kind, Node};

/// Parse the escaping policy directly without rewriting authored source.
fn escaped_document(source: String) -> markdown::Document {
    markdown::parse_with_options(
        source,
        markdown::MarkedOptions::Browser { escape_html: true },
    )
}

/// Visit every native node in source order, including table cells.
fn visit<'a>(nodes: &'a [Node], f: &mut impl FnMut(&'a Node)) {
    for node in nodes {
        f(node);
        match &node.kind {
            Kind::Paragraph(c)
            | Kind::Inline(c)
            | Kind::Heading(_, c, _)
            | Kind::List(_, c)
            | Kind::Item(c)
            | Kind::Quote(c, _)
            | Kind::Link(c, ..)
            | Kind::Strong(c)
            | Kind::Strike(c)
            | Kind::Emphasis(c) => visit(c, f),
            Kind::Table(table) => {
                for cell in std::iter::once(&table.header).chain(&table.rows).flatten() {
                    visit(&cell.children, f);
                }
            }
            _ => {}
        }
    }
}

/// Escaped markup inside otherwise raw directives, including malformed comment closers.
const DIRECTIVES: [(&str, &str); 4] = [
    (
        "<!-- --!><img src=x onerror=alert(1)>",
        "<!-- --!>&lt;img src=x onerror=alert(1)&gt;",
    ),
    (
        "a <!-- <b>x</b> --> tail",
        "<p>a <!-- &lt;b&gt;x&lt;/b&gt; --> tail</p>\n",
    ),
    (
        "<!thing <b>x</b>> **suffix**",
        "<!thing &lt;b&gt;x&lt;/b&gt;><p> <strong>suffix</strong></p>\n",
    ),
    (
        "<!thing [a](/a)> **suffix**",
        "<!thing [a](/a)><p> <strong>suffix</strong></p>\n",
    ),
];

/// Inputs whose escaping changes the structure around a native code span or block,
/// with the document HTML that shows every tag outside earlier stable code as text.
const SHIFTED: [(&str, &str); 7] = [
    (
        "<x y=\"`\"> `<img src=x onerror=alert(1)>`",
        "<p>&lt;x y=&quot;<code>&quot;&gt; </code>&lt;img src=x onerror=alert(1)&gt;`</p>\n",
    ),
    (
        "<x y='`'> `<img src=x onerror=alert(1)>`",
        "<p>&lt;x y=&#39;<code>&#39;&gt; </code>&lt;img src=x onerror=alert(1)&gt;`</p>\n",
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
    for (source, _) in SHIFTED.into_iter().chain(UNCLOSED).chain(DIRECTIVES) {
        let document = escaped_document(source.to_owned());
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

/// Independent native spans following code whose surrounding structure changes.
const SUFFIXES: [(&str, &str, Option<&str>); 5] = [
    ("`<i>`", "<p><code>&lt;i&gt;</code></p>\n", None),
    (
        "```html\n<i>\n```",
        "<div class=\"maestro-code-slot\" data-maestro-listing=\"0\"></div>\n",
        Some("<i>\n"),
    ),
    (
        "separator\n\n    <i>",
        "<p>separator</p>\n<div class=\"maestro-code-slot\" data-maestro-listing=\"0\"></div>\n",
        Some("<i>\n"),
    ),
    (
        "<https://example.com/a>",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" \
         href=\"https://example.com/a\">https://example.com/a</a></p>\n",
        None,
    ),
    (
        "<a@example.com>",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" \
         href=\"mailto:a@example.com\">a@example.com</a></p>\n",
        None,
    ),
];

#[test]
fn escaped_document_keeps_native_suffix_after_lost_span() {
    let lost: Vec<_> = SUFFIXES
        .iter()
        .filter_map(|(suffix, _, _)| {
            let source = format!("<pre>x</pre>\n    <img src=x>\n\n{suffix}");
            let document = escaped_document(source);
            (!document.source.ends_with(suffix)).then_some(*suffix)
        })
        .collect();
    assert!(lost.is_empty(), "rewritten native suffixes: {lost:?}");
}

#[test]
fn escaped_markup_keeps_native_suffix_after_lost_span() {
    let failures: Vec<_> = SUFFIXES
        .iter()
        .filter_map(|(suffix, html, payload)| {
            let source = format!("<pre>x</pre>\n    <img src=x>\n\n{suffix}");
            let rendered = prepare(&source, true);
            let expected = format!("<p>&lt;pre&gt;x&lt;/pre&gt;\n&lt;img src=x&gt;</p>\n{html}");
            let copied: Vec<_> = rendered
                .code
                .iter()
                .map(|code| code.code.as_str())
                .collect();
            let expected_copy: Vec<_> = payload.iter().copied().collect();
            (rendered.html != expected || copied != expected_copy).then_some((
                *suffix,
                rendered.html,
                copied.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            ))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "corrupted native suffixes: {failures:?}"
    );
}

#[test]
fn escaped_directives_keep_boundaries_and_inert_nested_tags() {
    let failures: Vec<_> = DIRECTIVES
        .into_iter()
        .enumerate()
        .filter_map(|(index, (source, expected))| {
            let actual = prepare(source, true).html;
            (actual != expected).then_some((index, actual, expected))
        })
        .collect();
    assert!(failures.is_empty(), "directive outputs: {failures:?}");
}

#[test]
fn display_neighbors_keep_authored_whitespace_once() {
    let html = prepare("before $$x$$ after", true).html;
    assert!(html.starts_with("<p>before </p>"), "{html}");
    assert!(html.ends_with("<p> after</p>\n"), "{html}");
    let html = prepare("| a |\n| - |\n| $x$ |", true).html;
    assert!(!html.contains("<td> "), "{html}");
    assert!(!html.contains(" </td>"), "{html}");
}

#[test]
fn display_block_definitions_resolve_forward_and_keep_first_identity() {
    for (source, href, count) in [
        ("$$x$$ [a]: /u\n\n[a]", "/u", 1),
        ("[a]\n\n$$x$$ [a]: /u\n\n[a]", "/u", 2),
        ("[a]: /first\n\n$$x$$ [a]: /u\n\n[a]", "/first", 1),
        ("before $$x$$ [a]: /u\n\n[a]", "/u", 1),
    ] {
        let html = prepare(source, true).html;
        assert_eq!(
            html.matches(&format!("href=\"{href}\"")).count(),
            count,
            "{html}"
        );
        assert!(!html.contains("[a]: /u"), "{html}");
    }
}

#[test]
fn mapped_entity_links_keep_browser_suffixes_once() {
    for entity in ["&amp;", "&#38;", "&#x26;"] {
        for frame in ["", "> ", "- "] {
            let html = prepare(&format!("{frame}https://x.y/a{entity}b tail"), true).html;
            let literal = format!("https://x.y/a&amp;{}b", &entity[1..]);
            assert!(
                html.contains(&format!("href=\"{literal}\">{literal}</a> tail")),
                "{html}"
            );
            assert_eq!(html.matches(" tail").count(), 1, "{html}");
        }
    }
}

#[test]
fn browser_list_continuations_keep_following_blocks_outside() {
    for first in ["# h", "    code", "```", "***"] {
        let html = prepare(&format!("- {first}\noutside"), true).html;
        assert!(html.ends_with("</ul>\n<p>outside</p>\n"), "{html}");
    }
    let html = prepare("- a\n\n\tb", true).html;
    assert_eq!(html, "<ul>\n<li><p>a</p>\n<p>  b</p>\n</li>\n</ul>\n");
}

#[test]
fn ordinary_browser_paragraphs_preserve_content_at_doubled_input() {
    for count in [32, 64] {
        let html = prepare(&"ordinary paragraph\n\n".repeat(count), true).html;
        assert_eq!(html, "<p>ordinary paragraph</p>\n".repeat(count));
    }
}

/// Exact returned HTML for native operands and complete code under deletion.
const DELETION_OPERANDS: [(&str, &str, &str); 14] = [
    (
        "a <b title=\"> `\"> ~~a~~ `",
        "<p>a <b title=\"> `\"> <del>a</del> `</p>\n",
        "<p>a &lt;b title=&quot;&gt; <code>&quot;&gt; ~~a~~ </code></p>\n",
    ),
    (
        "a [[b]](/u \"`\") ~~a~~ `",
        "<p>a <a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\" title=\"`\">[b]</a> <del>a</del> `</p>\n",
        "<p>a <a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\" title=\"`\">[b]</a> <del>a</del> `</p>\n",
    ),
    (
        "[label][r`] ~~a~~ `\n\n[r`]: /u",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\">label</a> <del>a</del> `</p>\n",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\">label</a> <del>a</del> `</p>\n",
    ),
    (
        "~~[a `b~~` c](/u)~~",
        "<p><del><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\">a <code>b~~</code> c</a></del></p>\n",
        "<p><del><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\">a <code>b~~</code> c</a></del></p>\n",
    ),
    (
        "~~a `b~~` c~~",
        "<p><del>a <code>b~~</code> c</del></p>\n",
        "<p><del>a <code>b~~</code> c</del></p>\n",
    ),
    (
        "~~[label](u~~)",
        "<p><del>[label](u</del>)</p>\n",
        "<p><del>[label](u</del>)</p>\n",
    ),
    (
        "~~[label](u~~",
        "<p><del>[label](u</del></p>\n",
        "<p><del>[label](u</del></p>\n",
    ),
    (
        "~~[label](u_(v~~",
        "<p><del>[label](u_(v</del></p>\n",
        "<p><del>[label](u_(v</del></p>\n",
    ),
    (
        "[label][r`] ~~a~~ `",
        "<p>[label][r<code>] ~~a~~ </code></p>\n",
        "<p>[label][r<code>] ~~a~~ </code></p>\n",
    ),
    (
        "~~a \\`b~~ c` x~~",
        "<p><del>a `b</del> c` x~~</p>\n",
        "<p><del>a `b</del> c` x~~</p>\n",
    ),
    (
        "~~<span title=\"`x~~`\">tail~~",
        "<p><del>&lt;span title=&quot;`x</del>`&quot;&gt;tail~~</p>\n",
        "<p><del>&lt;span title=&quot;<code>x~~</code>&quot;&gt;tail</del></p>\n",
    ),
    (
        "[~~a `b~~` c~~](/u)",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\"><del>a <code>b~~</code> c</del></a></p>\n",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\"><del>a <code>b~~</code> c</del></a></p>\n",
    ),
    (
        "~~[x](u~~ \"`\") ~~a~~ `",
        "<p><del>[x](u</del> &quot;<code>&quot;) ~~a~~ </code></p>\n",
        "<p><del>[x](u</del> &quot;<code>&quot;) ~~a~~ </code></p>\n",
    ),
    (
        "a <https://x.y/`> ~~a~~ `",
        "<p>a <a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://x.y/%60\">https://x.y/`</a> <del>a</del> `</p>\n",
        "<p>a <a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://x.y/%60\">https://x.y/`</a> <del>a</del> `</p>\n",
    ),
];

#[test]
fn native_operand_admission_preserves_browser_deletion_and_code() {
    for (source, raw, escaped) in DELETION_OPERANDS {
        assert_eq!(prepare(source, false).html, raw, "raw {source:?}");
        assert_eq!(prepare(source, true).html, escaped, "escaped {source:?}");
    }
}

#[test]
fn list_code_tabs_use_raw_lazy_and_aligned_spelling() {
    for (source, expected) in [
        (
            "- a\n`b\tc`",
            "<ul>\n<li>a\n<code>b\tc</code></li>\n</ul>\n",
        ),
        (
            "- a\n\t`b\tc`",
            "<ul>\n<li>a\n  <code>b    c</code></li>\n</ul>\n",
        ),
        (
            "- a\n`b\tc`\ntail",
            "<ul>\n<li>a\n<code>b\tc</code></li>\n</ul>\n<p>tail</p>\n",
        ),
    ] {
        for escaping in [false, true] {
            assert_eq!(prepare(source, escaping).html, expected, "{source:?}");
        }
    }
}
