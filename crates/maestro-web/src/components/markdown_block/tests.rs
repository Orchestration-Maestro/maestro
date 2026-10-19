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

/// Exact returned HTML for native operands and raw deletion closers.
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
        "<p><del>[a `b</del>` c](/u)~~</p>\n",
        "<p><del>[a `b</del>` c](/u)~~</p>\n",
    ),
    (
        "~~a `b~~` c~~",
        "<p><del>a `b</del>` c~~</p>\n",
        "<p><del>a `b</del>` c~~</p>\n",
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
        "<p><del>&lt;span title=&quot;`x</del>`&quot;&gt;tail~~</p>\n",
    ),
    (
        "[~~a `b~~` c~~](/u)",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\"><del>a `b</del>` c~~</a></p>\n",
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\"><del>a `b</del>` c~~</a></p>\n",
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
fn native_operand_boundaries_preserve_browser_deletion() {
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

#[test]
fn browser_deletion_reads_raw_source_after_unmatched_ticks() {
    for escaping in [false, true] {
        for (source, expected) in [
            ("``a ~~x~~ `", "<p>``a <del>x</del> `</p>\n"),
            (
                "~~[x](u~~ \"~~a `b~~` c~~\")",
                "<p><del>[x](u</del> &quot;<del>a `b</del>` c~~&quot;)</p>\n",
            ),
            (
                "~~<b title=\"u~~ ~~a `b~~` c~~\">",
                "<p><del>&lt;b title=&quot;u</del> <del>a `b</del>` c~~&quot;&gt;</p>\n",
            ),
        ] {
            assert_eq!(prepare(source, escaping).html, expected);
        }
    }
}

#[test]
fn browser_deletion_repeated_inputs_keep_reference_output() {
    for count in [32, 64] {
        for (source, expected) in [
            ("~ ", format!("<p>{}</p>\n", "~ ".repeat(count))),
            ("~~[a `b~~` c](/u)~~\n\n", "<p><del>[a `b</del>` c](/u)~~</p>\n".repeat(count)),
            ("a [[b]](/u \"`\") ~~a~~ `\n\n", "<p>a <a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/u\" title=\"`\">[b]</a> <del>a</del> `</p>\n".repeat(count)),
        ] {
            assert_eq!(prepare(&source.repeat(count), true).html, expected);
        }
    }
}

#[test]
fn native_math_preparation_keeps_delimiters_and_opaque_neighbors() {
    for (source, display) in [
        ("$x$", false),
        (r"\( x \)", false),
        ("$$x$$", true),
        (r"\[ x \]", true),
    ] {
        let html = prepare(source, true).html;
        assert!(html.contains("class=\"katex\""), "{source}: {html}");
        assert_eq!(html.contains("maestro-math-block"), display, "{source}");
        assert!(html.contains(">x</span>"), "{source}: {html}");
    }
    assert_eq!(
        prepare(r"`$x$` and \$x", true).html,
        "<p><code>$x$</code> and $x</p>\n"
    );
    let html = prepare("[value $x$](/a$x$ \"title $y$\")", true).html;
    assert!(html.starts_with(
        "<p><a target=\"_blank\" rel=\"noopener noreferrer\" href=\"/a$x$\" title=\"title $y$\">value "
    ));
    assert_eq!(html.matches("class=\"katex\"").count(), 1);
    assert!(html.ends_with("</a></p>\n"));
}

#[test]
fn native_document_preparation_preserves_structural_operands() {
    let cases = [
        (
            "## Title\n\n3. third\n4. fourth",
            "<h2>Title</h2>\n<ol start=\"3\">\n<li>third</li>\n<li>fourth</li>\n</ol>\n",
        ),
        (
            "- [x] done\n- [ ] next",
            "<ul>\n<li><input checked=\"\" disabled=\"\" type=\"checkbox\"> done</li>\n<li><input disabled=\"\" type=\"checkbox\"> next</li>\n</ul>\n",
        ),
        (
            "![*snow*](a%20b \"caption\")",
            "<p><img src=\"a%20b\" alt=\"snow\" title=\"caption\"></p>\n",
        ),
        (
            "one  \ntwo\n\n> **bold**",
            "<p>one<br>two</p>\n<blockquote>\n<p><strong>bold</strong></p>\n</blockquote>\n",
        ),
        (
            "| a | b |\n| :- | -: |\n| *x* | `y` |",
            "<div class=\"maestro-table-scroll\"><table>\n<thead>\n<tr>\n<th align=\"left\">a</th>\n<th align=\"right\">b</th>\n</tr>\n</thead>\n<tbody><tr>\n<td align=\"left\"><em>x</em></td>\n<td align=\"right\"><code>y</code></td>\n</tr>\n</tbody></table>\n</div>",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(prepare(source, true).html, expected, "{source}");
    }
}

#[test]
fn native_code_preparation_preserves_source_order_and_slot_identity() {
    let prepared = prepare(
        "data-maestro-listing\n\n```c++ extra\n<&amp;> 雪\n```\n\n```\nsecond\n```",
        true,
    );
    assert_eq!(prepared.slot_attribute, "data-maestro-listingx");
    assert_eq!(
        prepared.html,
        "<p>data-maestro-listing</p>\n<div class=\"maestro-code-slot\" data-maestro-listingx=\"0\"></div>\n<div class=\"maestro-code-slot\" data-maestro-listingx=\"1\"></div>\n"
    );
    let code: Vec<_> = prepared
        .code
        .iter()
        .map(|c| (c.language.as_str(), c.code.as_str()))
        .collect();
    assert_eq!(code, [("c++", "<&amp;> 雪\n"), ("text", "second\n")]);
}

#[test]
fn native_math_preparation_keeps_errors_and_container_boundaries() {
    let error = prepare(r"before $\frac{$ after", true).html;
    assert!(error.starts_with("<p>before "));
    assert!(error.contains("katex-error"));
    assert!(error.ends_with(" after</p>\n"));
    let quoted = prepare("> before $x$ after", true).html;
    assert!(quoted.starts_with("<blockquote>\n<p>before "));
    assert!(quoted.contains("class=\"katex\""));
    assert!(quoted.ends_with(" after</p>\n</blockquote>\n"));
    let table = prepare("| math |\n| - |\n| $x$ |", true).html;
    assert!(table.contains("<td><span class=\"katex\""));
    assert!(!table.contains("maestro-math-block"));
    let code = prepare("```text\n$x$\n```", true);
    assert!(!code.html.contains("katex"));
    assert_eq!(code.code[0].code, "$x$\n");
}

/// Spans that close inside a would-be formula, with the document HTML that keeps the span.
const SPAN_FIRST: [(&str, &str); 7] = [
    ("~~$~~$", "<p><del>$</del>$</p>\n"),
    ("~~old $5~~ new $3", "<p><del>old $5</del> new $3</p>\n"),
    ("~~a $b~~ c$ d", "<p><del>a $b</del> c$ d</p>\n"),
    ("~~$x~~$ y", "<p><del>$x</del>$ y</p>\n"),
    ("**$**$", "<p><strong>$</strong>$</p>\n"),
    ("**a $b** c$", "<p><strong>a $b</strong> c$</p>\n"),
    ("*a $*$ b*", "<p><em>a $</em>$ b*</p>\n"),
];

#[test]
fn spans_closing_inside_a_formula_keep_their_markup() {
    for (source, html) in SPAN_FIRST {
        assert_eq!(prepare(source, true).html, html, "{source}");
    }
    let resumed = prepare("~~$~~$ and $y$", true).html;
    assert!(resumed.starts_with("<p><del>$</del><span class=\"katex\""));
    assert!(resumed.ends_with("y$</p>\n"));
    let inside = prepare("$a ~~$ b $c~~ d$", true).html;
    assert_eq!(inside.matches("<span class=\"katex\">").count(), 2);
    assert!(!inside.contains("<del>"));
}

#[test]
fn admission_skips_a_formula_that_a_span_closes_inside() {
    let document = escaped_document("~~a $b~~ c$ d".to_owned());
    let context = markdown::InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    assert!(super::admission::scan(scope, scope.content_range(), false).is_empty());
}

#[test]
fn strike_emphasis_and_dollar_mixtures_never_panic() {
    // A strike opening after a formula that swallowed an earlier strike opener.
    prepare("$a ~~$ ~~b $c~~ d$", true);
    let pieces = [
        "~~", "~", "*", "**", "_", "$", "$", "a", " ", "`", "\\", "5", "\n",
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % 1_000).unwrap()
    };
    for _ in 0..1_000 {
        let length = next() % 14 + 1;
        let source: String = (0..length).map(|_| pieces[next() % pieces.len()]).collect();
        for escape in [true, false] {
            // A panic fails the test; rendered HTML is compared by the exact cases above.
            prepare(&source, escape);
        }
    }
}
