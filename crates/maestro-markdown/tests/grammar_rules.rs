#![cfg(test)]
//! Grammar observations through the owned parser's public entry points.
use maestro_markdown::{
    InlineAtom, InlineContext, Kind, MarkedOptions, Node, autolinks, parse, parse_with_options,
};
use serde::Deserialize;

#[path = "grammar_rules/observations.rs"]
mod observations;

/// One independently qualified native entity field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entity {
    /// Stable observation identity.
    id: String,
    /// Field's decoding or opacity policy.
    group: String,
    /// Authored field spelling.
    input: String,
    /// Independently recorded native value.
    expected: String,
}

/// Retain ordered text observations without deduplicating repeated values.
fn text(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| match &node.kind {
            Kind::Text(text) => text.decoded.clone(),
            Kind::Paragraph(children) | Kind::Link(children, ..) => text(children),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn named_numeric_and_opaque_entities_keep_native_values() {
    let rows: Vec<Entity> =
        serde_json::from_str(include_str!("fixtures/grammar_entities.json")).unwrap();
    for row in rows {
        let input = if row.group == "opaque-autolink-field" {
            format!("<{}>", row.input)
        } else {
            format!("v{}v", row.input)
        };
        let actual = text(&parse(input).nodes);
        let expected = if row.group == "opaque-autolink-field" {
            row.expected
        } else {
            format!("v{}v", row.expected)
        };
        assert_eq!(actual, expected, "{}", row.id);
    }
}

#[test]
fn typed_atoms_keep_surrounding_delimiters_and_original_operands() {
    let document = parse("*a $x$ b* [r][ref] `<m></m>`\n\n[ref]: /target".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let start = scope.text().find("$x$").unwrap();
    let run = scope.interpret(
        0..scope.text().trim_end_matches('\n').len(),
        std::slice::from_ref(&(start..start + 3)),
    );
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Emphasis(emphasis) = &children[0].kind else {
        panic!("emphasis around atom")
    };
    assert!(
        emphasis
            .iter()
            .any(|node| matches!(node.kind, Kind::Atom(0)))
    );
    let original = &scope.operands[0].1;
    assert!(
        run.atoms.iter().any(
            |atom| matches!(atom, InlineAtom::Resolved(node) if std::ptr::eq(*node, *original))
        )
    );
    let span = scope.operands[0].0.clone();
    let overlapping = scope.interpret(
        0..scope.text().trim_end_matches('\n').len(),
        std::slice::from_ref(&(span.start + 1..span.end - 1)),
    );
    assert!(
        !overlapping.atoms.iter().any(
            |atom| matches!(atom, InlineAtom::Resolved(node) if std::ptr::eq(*node, *original))
        )
    );
    assert!(
        overlapping
            .atoms
            .iter()
            .any(|atom| matches!(atom, InlineAtom::Opaque(0)))
    );
}

#[test]
fn inline_tags_preserve_link_and_raw_block_state() {
    observations::check("inline-html", 6);
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Browser { escape_html: false },
    ] {
        let document = parse_with_options("<!-- c -->\n\ntext".into(), options);
        assert!(matches!(&document.nodes[0].kind, Kind::HtmlBlock(html) if html == "<!-- c -->\n"));
        assert!(
            document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, Kind::Paragraph(_)))
        );
    }
}

#[test]
fn blank_lines_retain_block_boundaries() {
    observations::check("space", 6);
    let document = parse("\n\nalpha".into());
    assert_eq!(document.nodes[0].range, 0..2);
    assert_eq!(document.nodes[0].authored, "\n\n");
    assert!(matches!(document.nodes[0].kind, Kind::Gap));
    assert!(matches!(document.nodes[1].kind, Kind::Paragraph(_)));
    assert!(parse("\n\n  \n".into()).nodes.is_empty());
}

#[test]
fn line_breaks_distinguish_authored_and_decoded_newlines() {
    observations::check("break", 6);
    for (input, expected) in [
        ("a\n    b &NewLine; c", "a\n    b \n c"),
        ("a&#10;    b", "a\n    b"),
    ] {
        assert_eq!(text(&parse(input.into()).nodes), expected);
    }
    let document = parse("`x\n    y`".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[0].kind, Kind::Code(code) if code == "x     y"));
}

#[test]
fn lists_keep_marker_indent_looseness_and_task_state() {
    observations::check("list", 17);
    let document = parse("-\n- b".into());
    let Kind::List(None, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0].kind, Kind::Item(children) if children.is_empty()));
    let Kind::Item(children) = &items[1].kind else {
        panic!("item")
    };
    let Kind::Inline(inline) = &children[0].kind else {
        panic!("inline")
    };
    assert_eq!(text(inline), "b");
}

#[test]
fn references_use_document_index_and_first_definition() {
    observations::check("definition", 10);
    for (input, id, destination) in [
        ("[ẞ]\n\n[SS]: /u", "ẞ", "/u"),
        ("[ẞ]\n\n[ß]: /u", "ẞ", "/u"),
        ("[A B]\n\n[a  b]: /u", "A B", "/u"),
        ("[a][A  B]\n\n[a\tb]: /yes", "A B", "/yes"),
        ("[x]\n\n[x]: /first\n[x]: /second", "x", "/first"),
    ] {
        let document = parse(input.into());
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let Kind::Link(_, _, href, metadata) = &children[0].kind else {
            panic!("resolved reference")
        };
        assert_eq!(href, destination);
        assert_eq!(metadata.id, id);
    }
}

#[test]
fn links_keep_balanced_destinations_titles_and_image_children() {
    let document = parse("[a <https://x.y>](v) and <https://out.y>".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Link(label, _, href, _) = &children[0].kind else {
        panic!("outer anchor")
    };
    assert_eq!(href, "v");
    assert!(
        label
            .iter()
            .any(|node| matches!(&node.kind, Kind::Link(_, _, href, _) if href == "https://x.y"))
    );
    assert!(
        children
            .iter()
            .any(|node| matches!(&node.kind, Kind::Link(_, _, href, _) if href == "https://out.y"))
    );

    observations::check("link", 14);
    for input in [
        "[a](\\&amp; \"\\&amp;\")",
        "[a][r]\n\n[r]: \\&amp; \"\\&amp;\"",
    ] {
        let document = parse(input.into());
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let Kind::Link(_, _, href, metadata) = &children[0].kind else {
            panic!("link")
        };
        assert_eq!(href, "&amp;");
        assert_eq!(metadata.title, "&amp;");
    }
    let document = parse("| A |\n| - |\n| [x](https://a.b/a\\\\\\|b) |".into());
    let Kind::Table(table) = &document.nodes[0].kind else {
        panic!("table")
    };
    let Kind::Link(_, _, href, _) = &table.rows[0][0].children[0].kind else {
        panic!("cell link")
    };
    assert_eq!(href, "https://a.b/a\\|b");
    let document = parse("[a [b](u)](v)".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[0].kind, Kind::Text(value) if value.decoded == "[a "));
    assert!(children.iter().any(
        |node| matches!(&node.kind, Kind::Link(_, label, href, _) if label == "b" && href == "u")
    ));
    let control = parse("[a `[b]`](/v)".into());
    let Kind::Paragraph(children) = &control.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[0].kind, Kind::Link(_, _, href, _) if href == "/v"));
}

#[test]
fn fences_retain_body_info_and_closing_rules() {
    observations::check("fences", 7);
    for (input, expected) in [
        ("```lang\\+x &amp;\na\n```", "a\n"),
        ("```lang\\+x &amp;\na", "a"),
        ("```lang\\+x &amp;\na\n", "a\n"),
    ] {
        let document = parse(input.into());
        let Kind::CodeBlock(body, info) = &document.nodes[0].kind else {
            panic!("fenced code")
        };
        assert_eq!(
            (body.as_str(), info.as_deref()),
            (expected, Some("lang+x &"))
        );
    }
}

#[test]
fn tables_keep_column_alignment_pipes_and_authored_padding() {
    observations::check("table", 9);
    let document = parse("- item\n| A | B |\n| --- | --- |\n| 1 | 2 |".into());
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    assert_eq!(children.len(), 1);
    let Kind::Inline(children) = &children[0].kind else {
        panic!("lazy inline continuation")
    };
    assert_eq!(text(children), "item\n| A | B |\n| --- | --- |\n| 1 | 2 |");
    let document = parse("> q\n| A | B |\n| --- | --- |\n| 1 | 2 |".into());
    let Kind::Quote(children, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    assert_eq!(children.len(), 1);
    let Kind::Paragraph(children) = &children[0].kind else {
        panic!("lazy paragraph")
    };
    assert_eq!(text(children), "q\n| A | B |\n| --- | --- |\n| 1 | 2 |");
    for input in [
        "- | A | B |\n  | - | - |\n  | 1 | 2 |",
        "> | A | B |\n> | - | - |\n> | 1 | 2 |",
    ] {
        let document = parse(input.into());
        let nested = match &document.nodes[0].kind {
            Kind::Quote(children, _) => children,
            Kind::List(_, items) => match &items[0].kind {
                Kind::Item(children) => children,
                _ => panic!("item"),
            },
            _ => panic!("container"),
        };
        let Kind::Table(table) = &nested[0].kind else {
            panic!("explicit nested table")
        };
        assert_eq!(table.header.len(), 2);
        assert_eq!(text(&table.rows[0][0].children), "1");
        assert_eq!(text(&table.rows[0][1].children), "2");
    }
}

/// Observe actual selected destinations in source order, retaining repeated links.
fn hrefs(nodes: &[Node]) -> Vec<&str> {
    nodes
        .iter()
        .flat_map(|node| match &node.kind {
            Kind::Paragraph(children)
            | Kind::Inline(children)
            | Kind::Strong(children)
            | Kind::Emphasis(children)
            | Kind::Strike(children) => hrefs(children),
            Kind::Link(_, _, href, _) => vec![href.as_str()],
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn mapped_links_share_one_policy_aware_scanner() {
    observations::check("url", 10);
    for (options, expected) in [
        (
            MarkedOptions::Terminal,
            vec!["HTTPS://a.test", "ftp://b.test"],
        ),
        (
            MarkedOptions::Browser { escape_html: false },
            vec!["HTTPS://a.test", "ftp://b.test"],
        ),
        (
            MarkedOptions::Export,
            vec!["HTTPS://a.test", "ftp://b.test"],
        ),
    ] {
        let mut document = parse_with_options("HTTPS://a.test ftp://b.test".into(), options);
        assert!(hrefs(&document.nodes).is_empty());
        autolinks(&mut document);
        assert_eq!(hrefs(&document.nodes), expected);
    }
    for (options, expected) in [
        (MarkedOptions::Terminal, vec!["https://outer.test"]),
        (
            MarkedOptions::Browser { escape_html: false },
            vec!["https://outer.test"],
        ),
        (
            MarkedOptions::Browser { escape_html: true },
            vec!["https://inner.test", "https://outer.test"],
        ),
        (
            MarkedOptions::Export,
            vec!["https://inner.test", "https://outer.test"],
        ),
    ] {
        let mut document = parse_with_options(
            "a <a href=\"/raw\"> https://inner.test </a> https://outer.test".into(),
            options,
        );
        autolinks(&mut document);
        assert_eq!(hrefs(&document.nodes), expected);
    }
}

#[test]
fn quotes_keep_lazy_continuations_and_nested_extents() {
    observations::check("quote", 15);
    for input in ["> one\n> https://frame.test\n", "> https://frame.test\n"] {
        let mut document = parse(input.into());
        maestro_markdown::autolinks(&mut document);
        let Kind::Quote(children, _) = &document.nodes[0].kind else {
            panic!("quote")
        };
        let Kind::Paragraph(children) = &children[0].kind else {
            panic!("paragraph")
        };
        let link = children
            .iter()
            .find(|node| matches!(node.kind, Kind::Link(..)))
            .unwrap();
        assert_eq!(&document.source[link.range.clone()], "https://frame.test");
        assert_eq!(link.authored, "https://frame.test");
    }
    // A definition chunk adds no node, so the list it follows continues only after it.
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        let document = parse_with_options("> - a\n      p\n> [s]: /y\ne\n".into(), options);
        let Kind::Quote(children, _) = &document.nodes[0].kind else {
            panic!("quote")
        };
        assert!(matches!(children[0].kind, Kind::List(..)), "{options:?}");
        let last = children.last().unwrap();
        assert!(matches!(last.kind, Kind::Paragraph(_)), "{options:?}");
        assert_eq!(&document.source[last.range.clone()], "e\n", "{options:?}");
    }
    // A lazy definition line joins the quote paragraph once and defines nothing; a blank
    // quoted line after continued code still ends the paragraph.
    for (input, quoted, after) in [
        ("> q\n[r]: /x\n> y\n\n[r]\n", vec!["q\n[r]: /x\ny"], "[r]"),
        ("> q\n    code\n>\n> y\n\nz\n", vec!["q\ncode", "y"], "z"),
    ] {
        let document = parse(input.into());
        let Kind::Quote(children, _) = &document.nodes[0].kind else {
            panic!("quote")
        };
        let paragraphs: Vec<String> = children
            .iter()
            .filter(|node| matches!(node.kind, Kind::Paragraph(_)))
            .map(|node| text(std::slice::from_ref(node)))
            .collect();
        assert_eq!(paragraphs, quoted, "{input:?}");
        assert_eq!(text(&document.nodes[2..]), after, "{input:?}");
    }
}

#[test]
fn neighboring_marker_kinds_do_not_loosen_a_completed_list() {
    let document = parse("1. first\n2. second\n\n- third".into());
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    for item in items {
        let Kind::Item(children) = &item.kind else {
            panic!("item")
        };
        assert!(matches!(children[0].kind, Kind::Inline(_)));
    }
}

#[test]
fn headings_respect_delimiters_and_text_whitespace() {
    observations::check("heading", 9);
}

#[test]
fn html_terminators_retain_reachable_suffixes() {
    observations::check("html", 10);
}

#[test]
fn escapes_preserve_authored_and_decoded_units() {
    observations::check("escape", 5);
}

#[test]
fn emphasis_respects_unicode_flanking_masks_and_runs() {
    observations::check("emphasis", 12);
}

#[test]
fn code_spans_normalize_only_their_own_whitespace() {
    observations::check("codespan", 8);
}

#[test]
fn deletion_uses_each_callers_selected_rule() {
    observations::check("delete", 8);
}

#[test]
fn angle_links_stay_opaque_and_raw() {
    observations::check("autolink", 5);
}

#[test]
fn fallback_text_is_lossless_and_advances() {
    observations::check("text", 6);
}

/// Controlled extension responses for invalid-boundary validation.
struct InvalidExtension {
    hint: Option<usize>,
}
impl maestro_markdown::BlockTokenizer for InvalidExtension {
    fn start(&self, _: &str) -> Option<usize> {
        self.hint
    }
    fn tokenize(&mut self, _: &str) -> Option<maestro_markdown::BlockToken> {
        if self.hint.is_some() {
            None
        } else {
            Some(maestro_markdown::BlockToken {
                raw: String::new(),
                kind: "invalid".into(),
                text: String::new(),
            })
        }
    }
}

#[test]
fn block_extensions_reject_invalid_prefixes_and_hint_boundaries() {
    for (input, hint) in [("input", None), ("aé", Some(1)), ("input", Some(99))] {
        let result = maestro_markdown::parse_with_tokenizer(
            input.into(),
            MarkedOptions::Terminal,
            Some(&mut InvalidExtension { hint }),
        );
        assert!(matches!(result, Err(maestro_markdown::InvalidBlockToken)));
    }
}

#[test]
fn escaped_tag_interiors_keep_reference_emphasis_masks() {
    let document = parse_with_options(
        "<span title=\"<x> **a** [b](/docs)\">z</span>".into(),
        MarkedOptions::Browser { escape_html: true },
    );
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        children
            .iter()
            .any(|node| matches!(&node.kind, Kind::Text(text) if text.decoded.contains("**a**")))
    );
    assert!(
        !children
            .iter()
            .any(|node| matches!(node.kind, Kind::Strong(_)))
    );
    assert_eq!(hrefs(&document.nodes), ["/docs"]);
}

#[test]
fn export_masks_blocks_before_escaped_punctuation() {
    let document = parse_with_options(
        r#"\<span title="**a** [b](/docs)">z</span>"#.into(),
        MarkedOptions::Export,
    );
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        !children
            .iter()
            .any(|node| matches!(node.kind, Kind::Strong(_)))
    );
    assert_eq!(hrefs(&document.nodes), ["/docs"]);
}

#[test]
fn link_label_brackets_unescape_before_inline_parsing_with_original_ranges() {
    let input = r#"[\\[ $x$ ]](/docs "tip")"#;
    let document = parse_with_options(input.into(), MarkedOptions::Export);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Link(label, _, _, _) = &children[0].kind else {
        panic!("link")
    };
    assert_eq!(text(label), "[ $x$ ]");
    let original = &input[label[0].range.clone()];
    assert!(original.starts_with(r"\\["));
    let context = InlineContext::new(&document);
    let scope = context.scope(&children[0].range, true).unwrap();
    assert_eq!(scope.text(), r"\\[ $x$ ]");
    let run = scope.interpret(0..scope.text().len(), &[]);
    assert_eq!(text(&run.nodes), "[ $x$ ]");
}

#[test]
fn deletion_closes_on_raw_tildes_without_protecting_incomplete_links() {
    let document = parse_with_options("~~a `b~~` c~~".into(), MarkedOptions::Export);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Strike(children) = &children[0].kind else {
        panic!("strike")
    };
    assert_eq!(text(children), "a `b");
    let document = parse_with_options("~~[label](u~~)".into(), MarkedOptions::Export);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[0].kind, Kind::Strike(children) if text(children) == "[label](u"));
    assert_eq!(text(&children[1..]), ")");
    let document = parse_with_options(r"~~a \`b~~ c` x~~".into(), MarkedOptions::Export);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[0].kind, Kind::Strike(children) if text(children) == "a `b"));
}

#[test]
fn deletion_after_opaque_tags_ignores_their_ticks() {
    let document = parse("a <b title=\"`\"> ~~a~~ `".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        children
            .iter()
            .any(|node| matches!(node.kind, Kind::Strike(_)))
    );
}

#[test]
fn deletion_after_caller_atoms_ignores_their_ticks() {
    let document = parse("$`$ ~~a~~ `".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret(0..scope.text().len(), std::slice::from_ref(&(0..3)));
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(children[0].kind, Kind::Atom(0)));
    assert!(
        children
            .iter()
            .any(|node| matches!(node.kind, Kind::Strike(_)))
    );
}

#[test]
fn deletion_in_link_labels_closes_on_raw_tildes() {
    let document = parse("[~~a `b~~` c~~](/u)".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Link(children, ..) = &children[0].kind else {
        panic!("link")
    };
    let Kind::Strike(children) = &children[0].kind else {
        panic!("strike")
    };
    assert_eq!(text(children), "a `b");
}

/// Observe actual deletion and code leaves, including native link labels.
fn deletion_leaves(nodes: &[Node]) -> (Vec<&str>, Vec<&str>) {
    let mut strikes = Vec::new();
    let mut codes = Vec::new();
    for node in nodes {
        if let Kind::Code(code) = &node.kind {
            codes.push(code.as_str());
        }
        if let Kind::Strike(_) = &node.kind {
            strikes.push(node.authored.as_str());
        }
        match &node.kind {
            Kind::Paragraph(children)
            | Kind::Strike(children)
            | Kind::Link(children, ..)
            | Kind::Strong(children)
            | Kind::Emphasis(children) => {
                let (nested_strikes, nested_codes) = deletion_leaves(children);
                strikes.extend(nested_strikes);
                codes.extend(nested_codes);
            }
            _ => {}
        }
    }
    (strikes, codes)
}

#[test]
fn deletion_follows_native_operand_boundaries() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        for source in [
            "a [[b]](/u \"`\") ~~a~~ `",
            "[label][r`] ~~a~~ `\n\n[r`]: /u",
            "a <https://x.y/`> ~~a~~ `",
        ] {
            let document = parse_with_options(source.into(), options);
            assert_eq!(
                deletion_leaves(&document.nodes).0,
                ["~~a~~"],
                "{options:?} {source:?}"
            );
        }
        let source = "a <b title=\"> `\"> ~~a~~ `";
        let document = parse_with_options(source.into(), options);
        let expected = if matches!(
            options,
            MarkedOptions::Terminal | MarkedOptions::Browser { escape_html: false }
        ) {
            vec!["~~a~~"]
        } else {
            vec![]
        };
        assert_eq!(deletion_leaves(&document.nodes).0, expected, "{options:?}");
    }
}

#[test]
fn enclosing_deletion_uses_raw_closers_in_labels_and_tags() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        for source in [
            "~~[a `b~~` c](/u)~~",
            "~~a `b~~` c~~",
            "[~~a `b~~` c~~](/u)",
        ] {
            let document = parse_with_options(source.into(), options);
            let (strikes, codes) = deletion_leaves(&document.nodes);
            assert_eq!(strikes.len(), 1, "{source:?}");
            assert!(codes.is_empty(), "{options:?} {source:?}");
            assert!(strikes[0].ends_with("`b~~"));
        }
        for (source, strike) in [
            ("~~[label](u~~)", "~~[label](u~~"),
            ("~~[label](u~~", "~~[label](u~~"),
            ("~~[label](u_(v~~", "~~[label](u_(v~~"),
            (r"~~a \`b~~ c` x~~", r"~~a \`b~~"),
            ("~~[x](u~~ \"`\") ~~a~~ `", "~~[x](u~~"),
        ] {
            let document = parse_with_options(source.into(), options);
            assert_eq!(
                deletion_leaves(&document.nodes).0,
                [strike],
                "{options:?} {source:?}"
            );
        }
        let document = parse_with_options("[label][r`] ~~a~~ `".into(), options);
        assert!(deletion_leaves(&document.nodes).0.is_empty());
    }
    let source = "~~<span title=\"`x~~`\">tail~~";
    for options in [
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        let document = parse_with_options(source.into(), options);
        assert_eq!(
            deletion_leaves(&document.nodes),
            (vec!["~~<span title=\"`x~~"], vec![])
        );
    }
}

#[test]
fn deletion_reads_raw_source_after_unmatched_ticks() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        for (source, strikes) in [
            ("~~a `b~~` c~~", vec!["~~a `b~~"]),
            ("``a ~~x~~ `", vec!["~~x~~"]),
            (
                "~~[x](u~~ \"~~a `b~~` c~~\")",
                vec!["~~[x](u~~", "~~a `b~~"],
            ),
            (
                "~~<b title=\"u~~ ~~a `b~~` c~~\">",
                vec!["~~<b title=\"u~~", "~~a `b~~"],
            ),
        ] {
            let document = parse_with_options(source.into(), options);
            assert_eq!(
                deletion_leaves(&document.nodes).0,
                strikes,
                "{options:?} {source:?}"
            );
        }
    }
}
