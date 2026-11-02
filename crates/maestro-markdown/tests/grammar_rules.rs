#![cfg(test)]
//! Grammar observations through the owned parser's public entry points.
use maestro_markdown::{
    InlineAtom, InlineContext, Kind, MarkedOptions, Node, parse, parse_with_options,
};
use serde::Deserialize;

use crate::observations;

/// Compare an existing grammar family with its independently recorded targets.
fn check(group: &str, count: usize) {
    observations::check(
        group,
        count,
        include_str!("fixtures/grammar_observations.json"),
    );
}

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
fn spans_never_end_inside_a_caller_atom() {
    let document = parse("~~$~~$".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let start = scope.text().find("$~~$").unwrap();
    let run = scope.interpret(
        0..scope.text().trim_end_matches('\n').len(),
        std::slice::from_ref(&(start..start + 4)),
    );
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        !children
            .iter()
            .any(|node| matches!(node.kind, Kind::Strike(_)))
    );
    assert!(
        children
            .iter()
            .any(|node| matches!(node.kind, Kind::Atom(0)))
    );
}

#[test]
fn inline_tags_preserve_link_and_raw_block_state() {
    check("inline-html", 6);
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
    check("space", 9);
    let document = parse("\n\nalpha".into());
    assert_eq!(document.nodes[0].range, 0..2);
    assert_eq!(document.nodes[0].authored.to_owned_string(), "\n\n");
    assert!(matches!(document.nodes[0].kind, Kind::Gap));
    assert!(matches!(document.nodes[1].kind, Kind::Paragraph(_)));
    assert!(matches!(parse("\n\n  \n".into()).nodes[0].kind, Kind::Gap));
}

#[test]
fn line_breaks_distinguish_authored_and_decoded_newlines() {
    check("break", 6);
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
    check("list", 17);
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
    check("definition", 10);
    for (input, destination) in [
        ("[ẞ]\n\n[SS]: /u", "/u"),
        ("[ẞ]\n\n[ß]: /u", "/u"),
        ("[A B]\n\n[a  b]: /u", "/u"),
        ("[a][A  B]\n\n[a\tb]: /yes", "/yes"),
        ("[x]\n\n[x]: /first\n[x]: /second", "/first"),
    ] {
        let document = parse(input.into());
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let Kind::Link(_, _, href, _) = &children[0].kind else {
            panic!("resolved reference")
        };
        assert_eq!(href, destination);
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

    check("link", 14);
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
    check("fences", 7);
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
    check("table", 9);
    let document = parse("- item\n| A | B |\n| --- | --- |\n| 1 | 2 |".into());
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    assert_eq!(children.len(), 2);
    assert!(matches!(&children[1].kind, Kind::Table(table) if table.header.len() == 2));
    let document = parse("> q\n| A | B |\n| --- | --- |\n| 1 | 2 |".into());
    let Kind::Quote(children, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    assert_eq!(children.len(), 2);
    assert!(matches!(&children[1].kind, Kind::Table(table) if table.rows.len() == 1));
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
fn failed_local_runs_preserve_text_and_later_addresses() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        for (scalar, tail) in ['a', 'Z', '0', '.', '_', '+', '-']
            .into_iter()
            .flat_map(|scalar| ["", "@localhost", " b@c.io"].map(|tail| (scalar, tail)))
        {
            let input = format!("Payload: {}{tail}", scalar.to_string().repeat(256));
            let document = parse_with_options(input.clone(), options);

            assert_eq!(text(&document.nodes), input);
            let expected = if tail == " b@c.io" {
                vec!["mailto:b@c.io"]
            } else {
                vec![]
            };
            assert_eq!(hrefs(&document.nodes), expected);
            let Kind::Paragraph(children) = &document.nodes[0].kind else {
                panic!("paragraph")
            };
            for node in children {
                assert_eq!(
                    document.source[node.range.clone()],
                    node.authored.to_owned_string()
                );
            }
        }
    }
}

#[test]
fn mapped_links_share_one_policy_aware_scanner() {
    check("url", 17);
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
        let document = parse_with_options("HTTPS://a.test ftp://b.test".into(), options);

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
        let document = parse_with_options(
            "a <a href=\"/raw\"> https://inner.test </a> https://outer.test".into(),
            options,
        );

        assert_eq!(hrefs(&document.nodes), expected);
    }
}

#[test]
fn quotes_keep_lazy_continuations_and_nested_extents() {
    check("quote", 9);
    for input in ["> > a\n> b\ntail\n", "> - a\n>   b\nc\n"] {
        let document = parse(input.into());
        assert_eq!(document.nodes[0].authored.to_owned_string(), input);
        assert_eq!(&document.source[document.nodes[0].range.clone()], input);
    }
    for input in ["> one\n> https://frame.test\n", "> https://frame.test\n"] {
        let document = parse(input.into());

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
        assert_eq!(link.authored.to_owned_string(), "https://frame.test");
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
    check("heading", 9);
}

#[test]
fn html_terminators_retain_reachable_suffixes() {
    check("html", 10);
}

#[test]
fn escapes_preserve_authored_and_decoded_units() {
    check("escape", 5);
}

#[test]
fn emphasis_respects_unicode_flanking_masks_and_runs() {
    check("emphasis", 18);
}

#[test]
fn emphasis_flanking_distinguishes_combining_marks_from_letters_and_numbers() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        for (source, prefix, emphasized) in [
            ("α\u{0345}__a_", "α\u{0345}_", true),
            ("α__a_", "α__a_", false),
            ("1__a_", "1__a_", false),
            ("Ⅳ__a_", "Ⅳ__a_", false),
            ("²__a_", "²__a_", false),
        ] {
            let document = parse_with_options(source.into(), options);
            let Kind::Paragraph(children) = &document.nodes[0].kind else {
                panic!("paragraph")
            };
            assert_eq!(text(children), prefix, "{options:?} {source:?}");
            assert_eq!(children.len(), if emphasized { 2 } else { 1 });
            assert!(
                !emphasized
                    || matches!(&children[1].kind, Kind::Emphasis(content) if text(content) == "a")
            );
        }
    }
}

#[test]
fn code_spans_normalize_only_their_own_whitespace() {
    check("codespan", 8);
}

#[test]
fn deletion_uses_each_callers_selected_rule() {
    check("delete", 8);
}

#[test]
fn angle_links_stay_opaque_and_raw() {
    check("autolink", 5);
}

#[test]
fn fallback_text_is_lossless_and_advances() {
    check("text", 6);
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
        assert_eq!(
            result.err().unwrap().to_string(),
            "block tokenizer returned an invalid input prefix or start boundary"
        );
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
fn deletion_leaves(nodes: &[Node]) -> (Vec<String>, Vec<&str>) {
    let mut strikes = Vec::new();
    let mut codes = Vec::new();
    for node in nodes {
        if let Kind::Code(code) = &node.kind {
            codes.push(code.as_str());
        }
        if let Kind::Strike(_) = &node.kind {
            strikes.push(node.authored.to_owned_string());
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
            (vec!["~~<span title=\"`x~~".to_owned()], vec![])
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

#[test]
fn bare_addresses_follow_raw_anchor_transitions() {
    observations::check_with_options(
        "address-context",
        5,
        include_str!("fixtures/grammar_observations.json"),
        &[
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
        ],
    );
}

#[test]
fn documentation_grammar_keeps_emphasis_inside_bare_addresses() {
    let document = parse_with_options("https://a.b/_x_".into(), MarkedOptions::Documentation);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        children
            .iter()
            .any(|node| matches!(&node.kind, Kind::Emphasis(children) if text(children) == "x"))
    );
}

#[test]
fn unmatched_opening_parentheses_end_bare_addresses() {
    check("url-backpedal", 3);
}

#[test]
fn retained_raw_anchor_operands_keep_address_context() {
    let document = parse("<a href=\"x\">https://a.b/_x_</a>".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret(0..scope.text().len(), &[]);
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        children
            .iter()
            .any(|node| matches!(&node.kind, Kind::Emphasis(children) if text(children) == "x"))
    );
}

#[test]
fn caller_opaque_anchor_text_does_not_disable_bare_addresses() {
    let document = parse("<a href=\"x\"> https://a.b/_x_ </a>".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret(0..scope.text().len(), std::slice::from_ref(&(0..12)));
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(run.atoms[0], InlineAtom::Opaque(0)));
    assert!(
        !children
            .iter()
            .any(|node| matches!(node.kind, Kind::Emphasis(_)))
    );
    assert!(text(children).contains("https://a.b/_x_"));
}

#[test]
fn bare_addresses_follow_link_label_context() {
    check("address-label", 7);
}

#[test]
fn mapped_addresses_resume_after_link_and_image_labels() {
    observations::check_with_options(
        "address-completion",
        2,
        include_str!("fixtures/grammar_observations.json"),
        &[
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
        ],
    );
}

#[test]
fn image_labels_disable_address_tokens() {
    let document = parse("![https://a.b/_x_](y)".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Image(_, metadata) = &children[0].kind else {
        panic!("image")
    };
    assert!(
        matches!(&metadata.children[1].kind, Kind::Emphasis(children) if text(children) == "x")
    );
}

#[test]
fn link_label_scopes_disable_address_tokens() {
    let document = parse("[https://a.b/_x_](y)".into());
    let context = InlineContext::new(&document);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let scope = context.scope(&children[0].range, true).unwrap();
    let run = scope.interpret(0..scope.text().len(), &[]);
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(matches!(&children[1].kind, Kind::Emphasis(children) if text(children) == "x"));
}

#[test]
fn image_label_link_completion_resumes_address_tokens() {
    let document = parse("![[b](z) https://a.b/_x_](y)".into());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Image(_, metadata) = &children[0].kind else {
        panic!("image")
    };
    assert!(matches!(&metadata.children[0].kind, Kind::Link(_, _, href, _) if href == "z"));
    assert_eq!(text(&metadata.children), "b https://a.b/_x_");
    assert!(
        !metadata
            .children
            .iter()
            .any(|node| matches!(node.kind, Kind::Emphasis(_)))
    );
}

#[test]
fn retained_labels_and_display_splits_share_address_context() {
    for (source, label, emphasis) in [
        (
            "<a href=\"x\">[l](y) $$x$$ https://a.b/_x_</a>",
            false,
            false,
        ),
        (
            "<a href=\"x\">![l](y) $$x$$ https://a.b/_x_</a>",
            false,
            false,
        ),
        ("[![b](z) $$x$$ https://a.b/_x_](y)", true, false),
        ("<a href=\"x\"> $$x$$ https://a.b/_x_</a>", false, true),
        (
            "<a href=\"x\"><https://c.d> $$x$$ https://a.b/_x_</a>",
            false,
            true,
        ),
        (
            "<a href=\"x\"><b@c.d> $$x$$ https://a.b/_x_</a>",
            false,
            true,
        ),
    ] {
        let document = parse(source.into());
        let context = InlineContext::new(&document);
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let owner = if label {
            &children[0]
        } else {
            &document.nodes[0]
        };
        let scope = context.scope(&owner.range, label).unwrap();
        let start = scope.text().find("$$x$$").unwrap();
        let formula = start..start + 5;
        for range in [0..scope.text().len(), formula.end..scope.text().len()] {
            let opaque = if range.start == 0 {
                vec![formula.clone()]
            } else {
                vec![]
            };
            let run = scope.interpret(range.clone(), &opaque);
            let Kind::Paragraph(children) = &run.nodes[0].kind else {
                panic!("paragraph")
            };
            assert_eq!(
                children
                    .iter()
                    .any(|node| matches!(node.kind, Kind::Emphasis(_))),
                emphasis,
                "{source:?} {range:?}"
            );
            assert!(
                emphasis || retained_text(children, &run.atoms).contains("https://a.b/_x_"),
                "{source:?} {range:?}"
            );
        }
    }
}

/// Read displayed text through the original operands retained by local interpretation.
fn retained_text(nodes: &[Node], atoms: &[InlineAtom<'_>]) -> String {
    nodes
        .iter()
        .map(|node| {
            let original = match node.kind {
                Kind::Atom(index) => match &atoms[index] {
                    InlineAtom::Resolved(original) => *original,
                    InlineAtom::Opaque(_) => node,
                },
                _ => node,
            };
            text(std::slice::from_ref(original))
        })
        .collect()
}

/// Dollar prefixes used to observe the shared admission contract, not math rendering.
#[derive(Default)]
struct CursorDollar {
    /// Selected authored prefixes in callback order.
    selected: Vec<String>,
}
impl maestro_markdown::InlineTokenizer for CursorDollar {
    fn start(&self, source: &str) -> Option<usize> {
        source.find('$')
    }
    fn tokenize(&mut self, source: &str) -> Option<maestro_markdown::InlineToken> {
        let end = 1 + source.strip_prefix('$')?.find('$')?;
        if end == 1 {
            return None;
        }
        let index = self.selected.len();
        self.selected.push(source[..=end].to_owned());
        Some(maestro_markdown::InlineToken {
            length: end + 1,
            index,
        })
    }
}

/// Observe selected spans, literal neighbors and caller identities without rendering math.
fn cursor_shape(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| match &node.kind {
            Kind::Paragraph(children) => cursor_shape(children),
            Kind::Text(text) => text.decoded.clone(),
            Kind::Atom(index) => format!("@{index}"),
            Kind::Emphasis(children) => format!("<em>{}</em>", cursor_shape(children)),
            Kind::Strong(children) => format!("<strong>{}</strong>", cursor_shape(children)),
            Kind::Strike(children) => format!("<del>{}</del>", cursor_shape(children)),
            _ => panic!("unexpected inline kind"),
        })
        .collect()
}

#[test]
fn cursor_admission_leaves_delimiters_visible() {
    for (source, expected, selected) in [
        ("_$__a**$ bx y a_", "_@0 bx y a_", "$__a**$"),
        ("x y~~*$**$**", "x y~~<em>@0*</em>", "$**$"),
    ] {
        let document =
            parse_with_options(source.into(), MarkedOptions::Browser { escape_html: true });
        let context = InlineContext::new(&document);
        let scope = context.scope(&document.nodes[0].range, false).unwrap();
        let mut tokenizer = CursorDollar::default();
        let run = scope.interpret_with_tokenizer(scope.content_range(), &mut tokenizer);
        assert_eq!(cursor_shape(&run.nodes), expected, "{source}");
        assert_eq!(tokenizer.selected, [selected], "{source}");
        assert!(matches!(run.atoms.as_slice(), [InlineAtom::Opaque(0)]));
    }
}

#[test]
fn cursor_admission_respects_recursive_windows() {
    for (source, expected, selected) in [
        ("~~a $b~~ c$ d", "<del>a $b</del> c$ d", Vec::<&str>::new()),
        ("**a $b** c$", "<strong>a $b</strong> c$", vec![]),
        ("*a $*$ b*", "<em>a $</em>$ b*", vec![]),
        (
            "***a $b$ c***",
            "<em><strong>a @0 c</strong></em>",
            vec!["$b$"],
        ),
        ("~~*a $b$ c*~~", "<del><em>a @0 c</em></del>", vec!["$b$"]),
    ] {
        let document =
            parse_with_options(source.into(), MarkedOptions::Browser { escape_html: true });
        let context = InlineContext::new(&document);
        let scope = context.scope(&document.nodes[0].range, false).unwrap();
        let mut tokenizer = CursorDollar::default();
        let run = scope.interpret_with_tokenizer(scope.content_range(), &mut tokenizer);
        assert_eq!(cursor_shape(&run.nodes), expected, "{source}");
        assert_eq!(tokenizer.selected, selected, "{source}");
    }
}

#[test]
fn static_caller_atoms_keep_their_contents_hidden() {
    let document = parse_with_options(
        "_$__a**$ bx y a_".into(),
        MarkedOptions::Browser { escape_html: true },
    );
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret(scope.content_range(), std::slice::from_ref(&(1..8)));
    assert_eq!(cursor_shape(&run.nodes), "<em>@0 bx y a</em>");
}

/// Return a run after its temporary context and caller grammar have been released.
fn retained_cursor_run(document: &maestro_markdown::Document) -> maestro_markdown::InlineRun<'_> {
    let context = InlineContext::new(document);
    let mut tokenizer = CursorDollar::default();
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    scope.interpret_with_tokenizer(scope.content_range(), &mut tokenizer)
}

#[test]
fn cursor_runs_keep_independent_retained_operands() {
    let first = parse("$a$ [r](u)".into());
    let second = parse("$b$ [s](v)".into());
    let a = retained_cursor_run(&first);
    let b = retained_cursor_run(&second);
    for (result, destination, document) in [(&a, "u", &first), (&b, "v", &second)] {
        let original = result
            .atoms
            .iter()
            .find_map(|atom| match atom {
                InlineAtom::Resolved(node) => Some(*node),
                InlineAtom::Opaque(_) => None,
            })
            .unwrap();
        assert!(matches!(&original.kind, Kind::Link(_, _, href, _) if href == destination));
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let retained = children
            .iter()
            .find(|node| matches!(node.kind, Kind::Link(..)))
            .unwrap();
        assert!(std::ptr::eq(original, retained));
        assert!(
            result
                .atoms
                .iter()
                .any(|atom| matches!(atom, InlineAtom::Opaque(0)))
        );
    }
}

#[test]
fn cursor_label_source_uses_unescaped_brackets() {
    for escape_html in [true, false] {
        let document = parse_with_options(
            r"[$a\]b$](u)".into(),
            MarkedOptions::Browser { escape_html },
        );
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("paragraph")
        };
        let link = children
            .iter()
            .find(|node| matches!(node.kind, Kind::Link(..)))
            .unwrap();
        let context = InlineContext::new(&document);
        let scope = context.scope(&link.range, true).unwrap();
        let mut tokenizer = CursorDollar::default();
        let run = scope.interpret_with_tokenizer(scope.content_range(), &mut tokenizer);
        assert_eq!(tokenizer.selected, ["$a]b$"]);
        let Kind::Paragraph(children) = &run.nodes[0].kind else {
            panic!("paragraph")
        };
        assert_eq!(children[0].authored.to_owned_string(), "$a]b$");
        assert_eq!(children[0].range, 1..7);
        assert!(matches!(&link.kind, Kind::Link(_, _, href, _) if href == "u"));
    }
}

/// Deliberately invalid native byte lengths and hints.
struct CursorPrefix {
    /// Requested length, without a producer guarantee.
    length: usize,
    /// Requested hint offset.
    hint: Option<usize>,
}
impl maestro_markdown::InlineTokenizer for CursorPrefix {
    fn start(&self, _: &str) -> Option<usize> {
        self.hint
    }
    fn tokenize(&mut self, _: &str) -> Option<maestro_markdown::InlineToken> {
        Some(maestro_markdown::InlineToken {
            length: self.length,
            index: 0,
        })
    }
}

#[test]
fn cursor_admission_checks_native_prefix_boundaries() {
    let document = parse("é".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    for (length, hint) in [
        (0, None),
        (1, None),
        (3, None),
        (usize::MAX, Some(usize::MAX)),
    ] {
        let run = scope
            .interpret_with_tokenizer(scope.content_range(), &mut CursorPrefix { length, hint });
        assert!(run.atoms.is_empty());
        assert_eq!(cursor_shape(&run.nodes), "é");
    }
    let run = scope.interpret_with_tokenizer(
        scope.content_range(),
        &mut CursorPrefix {
            length: 2,
            hint: None,
        },
    );
    assert_eq!(cursor_shape(&run.nodes), "@0");
    assert_eq!(run.atoms.len(), 1);
    let document = parse("xé".into());
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret_with_tokenizer(
        scope.content_range(),
        &mut CursorPrefix {
            length: 0,
            hint: Some(1),
        },
    );
    assert!(run.atoms.is_empty());
    assert_eq!(cursor_shape(&run.nodes), "xé");
}

/// One source input with independently recorded ordered operands for every caller.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkSource {
    /// Stable failure attribution.
    id: String,
    /// Complete authored source.
    input: String,
    /// Terminal, raw browser, escaped browser and export operands.
    targets: [Vec<(String, String)>; 4],
}

#[test]
fn mapped_link_sources_match_selected_grammars() {
    let rows: Vec<LinkSource> =
        serde_json::from_str(include_str!("fixtures/link_sources.json")).unwrap();
    assert_eq!(rows.len(), 84);
    let mut failures = Vec::new();
    for row in rows {
        for (options, expected) in [
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
            MarkedOptions::Export,
        ]
        .into_iter()
        .zip(row.targets)
        {
            let document = parse_with_options(row.input.clone(), options);

            assert_eq!(document.source, row.input, "{} {options:?}", row.id);
            let (mut labels, mut destinations) = (Vec::new(), Vec::new());
            observations::metadata(
                &document.nodes,
                &mut Vec::new(),
                &mut labels,
                &mut destinations,
                options,
            );
            let actual: Vec<_> = labels.into_iter().zip(destinations).collect();
            if actual != expected {
                failures.push(format!(
                    "{} {options:?}: {actual:?} != {expected:?}",
                    row.id
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn rejected_folded_web_spans_keep_nested_candidates() {
    for prefix in ["HTTPS://", "HTTP://", "FTP://", "WWW."] {
        for (label, destination) in [
            ("https://target.example", "https://target.example"),
            ("http://target.example", "http://target.example"),
            ("ftp://target.example", "ftp://target.example"),
            ("www.target.example", "http://www.target.example"),
        ] {
            let source = format!("See {prefix}redirect.example/?url={label}");
            for options in [MarkedOptions::Terminal, MarkedOptions::Export] {
                let document = parse_with_options(source.clone(), options);

                let (mut labels, mut destinations) = (Vec::new(), Vec::new());
                observations::metadata(
                    &document.nodes,
                    &mut Vec::new(),
                    &mut labels,
                    &mut destinations,
                    options,
                );
                assert_eq!(document.source, source);
                assert_eq!(labels, [label], "{source} {options:?}");
                assert_eq!(destinations, [destination], "{source} {options:?}");
            }
        }
    }
}

#[test]
fn inline_admission_keeps_mixed_case_links_after_escaped_rejected_prefixes() {
    for options in [MarkedOptions::Terminal, MarkedOptions::Export] {
        for tail in ["", "/?url=https://last.example"] {
            let input = format!(r"See HTTPS://outer.example/\*HTTPS://target.example{tail}");
            let document = parse_with_options(input.clone(), options);
            assert_eq!(
                hrefs(&document.nodes),
                [format!("HTTPS://target.example{tail}")],
                "{options:?} {input:?}"
            );
        }
    }
}

#[test]
fn fallback_text_keeps_multiline_spelling_and_reached_cursors() {
    for options in [
        MarkedOptions::Documentation,
        MarkedOptions::Terminal,
        MarkedOptions::Export,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
    ] {
        for source in ["plain line.\n".repeat(256), "a_ ".repeat(256)] {
            let document = parse_with_options(source.clone(), options);
            let Kind::Paragraph(children) = &document.nodes[0].kind else {
                panic!("paragraph")
            };
            let decoded: String = children
                .iter()
                .map(|node| match &node.kind {
                    Kind::Break => "\n".to_owned(),
                    _ => text(std::slice::from_ref(node)),
                })
                .collect();
            assert_eq!(decoded, source.trim_end_matches('\n'));
        }
        let document = parse_with_options("a\nHTTPS://a.b".into(), options);
        let expected = if matches!(
            options,
            MarkedOptions::Browser { .. } | MarkedOptions::Export
        ) {
            vec!["HTTPS://a.b"]
        } else {
            vec![]
        };
        assert_eq!(hrefs(&document.nodes), expected, "{options:?}");
    }
}

#[test]
fn cursor_admission_consumes_later_html_without_entering_opaque_operands() {
    let source = "$x <img src=x onerror=alert(1)>$";
    let document = parse_with_options(source.into(), MarkedOptions::Browser { escape_html: false });
    let context = InlineContext::new(&document);
    let scope = context.scope(&document.nodes[0].range, false).unwrap();
    let run = scope.interpret_with_tokenizer(scope.content_range(), &mut CursorDollar::default());
    let Kind::Paragraph(children) = &run.nodes[0].kind else {
        panic!("paragraph");
    };
    assert_eq!(
        children.len(),
        1,
        "earlier caller consumes the later HTML operand"
    );
    let Kind::Atom(index) = children[0].kind else {
        panic!("caller atom");
    };
    assert!(matches!(run.atoms[index], InlineAtom::Opaque(0)));
    assert_eq!(children[0].authored.to_owned_string(), source);
    for source in [
        "$x `y$`",
        "$x ![y$](u)",
        "$x <https://example.test/y$>",
        "before <i title='$x$'>kept</i>",
    ] {
        let document =
            parse_with_options(source.into(), MarkedOptions::Browser { escape_html: false });
        let context = InlineContext::new(&document);
        let scope = context.scope(&document.nodes[0].range, false).unwrap();
        let run =
            scope.interpret_with_tokenizer(scope.content_range(), &mut CursorDollar::default());
        let Kind::Paragraph(children) = &run.nodes[0].kind else {
            panic!("paragraph");
        };
        assert!(!children.iter().any(|node| matches!(node.kind, Kind::Atom(i) if matches!(run.atoms[i], InlineAtom::Opaque(_)))), "opaque control");
    }
}

/// Compare delimiter-run families through every caller grammar selection.
#[test]
fn delimiter_runs_keep_caller_flanking_and_nesting() {
    observations::check_with_options(
        "delimiter-runs",
        1441,
        include_str!("fixtures/delimiter_runs.json"),
        &delimiter_options(),
    );
}

/// Recursive windows can reveal syntax hidden by their enclosing mask.
#[test]
fn recursive_delimiter_windows_keep_inner_syntax() {
    observations::check_with_options(
        "delimiter-windows",
        13,
        include_str!("fixtures/delimiter_runs.json"),
        &delimiter_options(),
    );
}

/// Caller policies sharing delimiter recognition and source-window selection.
fn delimiter_options() -> [MarkedOptions; 5] {
    [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
        MarkedOptions::Documentation,
    ]
}
