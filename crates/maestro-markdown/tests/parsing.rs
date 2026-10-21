#![cfg(test)]

use maestro_markdown::{Alignment, Kind, parse};

#[test]
fn parser_keeps_owned_browser_metadata() {
    let document = {
        let input = String::from(
            "[**label**][ref] ![a `b`](image \"tip\")\n\n[ref]: /path \"title\"\n\n| a | b |\n| :- | -: |\n| x | y |\n\n- [x] yes\n- [ ] no\n",
        );
        parse(input)
    };
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let Kind::Link(label, _, href, metadata) = &children[0].kind else {
        panic!("link")
    };
    assert_eq!(href, "/path");
    assert_eq!(metadata.title, "title");
    assert!(matches!(label[0].kind, Kind::Strong(_)));
    let Kind::Image(_, image) = &children[2].kind else {
        panic!("image")
    };
    assert_eq!(image.destination, "image");
    assert_eq!(image.title, "tip");
    assert!(matches!(image.children[1].kind, Kind::Code(_)));
    let table = document
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            Kind::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();
    assert_eq!(table.alignments, [Alignment::Left, Alignment::Right]);
    assert_tasks(&document);
}

#[test]
fn source_framed_images_keep_code_and_document_references() {
    let document = {
        let input =
            String::from("> ![a `[b]` [c][r]][img]\n\n[r]: /c \"child\"\n[img]: /img \"tip\"\n");
        parse(input)
    };
    let Kind::Quote(quote, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    let Kind::Paragraph(children) = &quote[0].kind else {
        panic!("paragraph")
    };
    let image = &children[0];
    assert_eq!(image.authored, "![a `[b]` [c][r]][img]");
    assert_eq!(
        &document.source[image.range.clone()],
        "![a `[b]` [c][r]][img]"
    );
    let Kind::Image(_, metadata) = &image.kind else {
        panic!("image")
    };
    assert_eq!(metadata.destination, "/img");
    assert_eq!(metadata.title, "tip");
    assert_image_reference_children(metadata);
}

/// Check opaque code and the separately resolved definition inside an image label.
fn assert_image_reference_children(metadata: &maestro_markdown::ImageMetadata) {
    assert!(matches!(&metadata.children[1].kind, Kind::Code(code) if code == "[b]"));
    let Kind::Link(label, _, href, child) = &metadata.children[3].kind else {
        panic!("reference child")
    };
    assert_eq!(href, "/c");
    assert_eq!(child.title, "child");
    assert!(matches!(&label[0].kind, Kind::Text(text) if text.decoded == "c"));
}

#[test]
fn parser_retains_container_source_extents() {
    let document = parse(
        "> $$x\n> y$$\n\n- a \\*b\\* `opaque`\n  tail\n\n| a | b |\n| - | - |\n| \\| | &amp; |\n"
            .to_owned(),
    );
    let Kind::Quote(quote, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    assert_eq!(quote[0].authored, "$$x\ny$$");
    assert_eq!(&document.source[quote[0].range.clone()], "$$x\n> y$$");
    let Kind::List(_, items) = &document.nodes[2].kind else {
        panic!("list")
    };
    let Kind::Item(item) = &items[0].kind else {
        panic!("item")
    };
    let Kind::Inline(children) = &item[0].kind else {
        panic!("inline")
    };
    assert_eq!(item[0].authored, "a \\*b\\* `opaque`\ntail");
    assert!(
        children
            .iter()
            .any(|node| matches!(&node.kind, Kind::Code(text) if text == "opaque"))
    );
    let table = document
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            Kind::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();
    assert_eq!(table.rows[0][0].authored, " | ");
    assert_eq!(&document.source[table.rows[0][0].range.clone()], " \\| ");
    let Kind::Text(text) = &table.rows[0][1].children[0].kind else {
        panic!("text")
    };
    assert_eq!(text.authored, "&amp;");
    assert_eq!(text.decoded, "&");
}

#[test]
fn parser_retains_native_structure_with_inline_links() {
    let document = parse("$x$ https://example.org/$q$ &amp; www.example.org. \\* https://other.org\n\n| a |\n| - |\n| www.example.org/\\| |\n".to_owned());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    let links: Vec<_> = children
        .iter()
        .filter_map(|node| match &node.kind {
            Kind::Link(_, label, href, _) => Some((node, label, href)),
            _ => None,
        })
        .collect();
    assert_eq!(links.len(), 3);
    assert_eq!(
        &document.source[links[0].0.range.clone()],
        "https://example.org/$q$"
    );
    assert_eq!(links[0].1, "https://example.org/$q$");
    assert_eq!(links[1].2, "http://www.example.org");
    let text: String = children
        .iter()
        .filter_map(|node| match &node.kind {
            Kind::Text(text) => Some(text.decoded.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "$x$  & . * ");
    let table = document
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            Kind::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();
    let Kind::Link(_, label, href, _) = &table.rows[0][0].children[0].kind else {
        panic!("link")
    };
    assert_eq!(label, "www.example.org/|");
    assert_eq!(href, "http://www.example.org/|");
}

fn assert_tasks(document: &maestro_markdown::Document) {
    let list = document
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            Kind::List(_, items) => Some(items),
            _ => None,
        })
        .unwrap();
    for (item, expected) in list.iter().zip([true, false]) {
        let Kind::Item(children) = &item.kind else {
            panic!("item")
        };
        assert!(matches!(children[0].kind, Kind::TaskListMarker(checked) if checked == expected));
    }
}

#[test]
fn mapped_links_retain_source_boundaries_before_escaped_math() {
    let document =
        parse("https://example.test/a $x$ www.example.test \\(y\\) a@example.test".into());

    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    for node in children {
        if let Kind::Link(..) = &node.kind {
            assert_eq!(&document.source[node.range.clone()], node.authored);
        }
    }
}

#[test]
fn inline_context_keeps_original_reference_operands_and_consumed_suffixes() {
    use maestro_markdown::{InlineAtom, InlineContext};
    for label in ["[$x$]", "[$x$][]", "[$x$][r]"] {
        let document = parse(format!(
            "{label}\n\n[$x$]: /docs \"tip\"\n[r]: /docs \"tip\""
        ));
        let context = InlineContext::new(&document);
        let scope = context.scope(&document.nodes[0].range, false).unwrap();
        let run = scope.interpret(0..scope.text().len(), &[]);
        assert_eq!(run.atoms.len(), 1);
        let InlineAtom::Resolved(node) = run.atoms[0] else {
            panic!("resolved anchor")
        };
        let Kind::Link(_, _, href, meta) = &node.kind else {
            panic!("anchor")
        };
        assert_eq!(href, "/docs");
        assert_eq!(meta.title, "tip");
        assert_eq!(&scope.text()[scope.operands[0].0.clone()], label);
        let label = context.scope(&node.range, true).unwrap();
        assert_eq!(label.text(), "$x$");
        let run = label.interpret(0..3, std::slice::from_ref(&(0..3)));
        assert!(matches!(run.atoms[0], InlineAtom::Opaque(0)));
        let Kind::Paragraph(c) = &run.nodes[0].kind else {
            panic!("inline paragraph")
        };
        assert!(matches!(c[0].kind, Kind::Atom(0)));
    }
}

#[test]
fn inline_scopes_map_container_prefixes_escaped_labels_and_table_pipes() {
    use maestro_markdown::InlineContext;
    let document = parse("> - [\\[$x$\\]][r] and \\(x\n>   y\\)\n\n[r]: /docs\n\n| cell |\n| - |\n| https://example.test/a\\|b/$x$ $y$ |".into());

    let context = InlineContext::new(&document);
    let Kind::Quote(c, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    let Kind::List(_, c) = &c[0].kind else {
        panic!("list")
    };
    let Kind::Item(c) = &c[0].kind else {
        panic!("item")
    };
    let scope = context.scope(&c[0].range, false).unwrap();
    assert_eq!(scope.text(), "[\\[$x$\\]][r] and \\(x\ny\\)");
    let label = context.scope(&scope.operands[0].1.range, true).unwrap();
    assert_eq!(label.text(), "\\[$x$\\]");
    let table = document
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            Kind::Table(t) => Some(t),
            _ => None,
        })
        .unwrap();
    let scope = context.scope(&table.rows[0][0].range, false).unwrap();
    assert_eq!(scope.text(), " https://example.test/a|b/$x$ $y$ ");
    assert_eq!(
        &scope.text()[scope.content_range()],
        "https://example.test/a|b/$x$ $y$"
    );
    assert_eq!(
        &scope.text()[scope.operands[0].0.clone()],
        "https://example.test/a|b/$x$"
    );
}

#[test]
fn mapped_links_keep_entity_units_and_suffix_source_ranges() {
    for entity in ["&amp;", "&#38;", "&#x26;"] {
        for (frame, source_tail, decoded_tail) in [
            ("", " tail", " tail"),
            ("> ", "\n> tail", "\ntail"),
            ("- ", "\n  tail", "\ntail"),
        ] {
            assert_mapped_entity_link(entity, frame, source_tail, decoded_tail);
        }
    }
}

/// Observe a complete mapped link and its decoded suffix in one source frame.
fn assert_mapped_entity_link(entity: &str, frame: &str, source_tail: &str, decoded_tail: &str) {
    let label = format!("https://x.y/a{entity}b");
    let document = parse(format!("{frame}{label}{source_tail}"));

    let mut nodes = document.nodes.as_slice();
    let nodes = loop {
        nodes = match &nodes[0].kind {
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => children,
            Kind::Paragraph(children) | Kind::Inline(children) => break children,
            _ => panic!("container"),
        };
    };
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].range, frame.len()..frame.len() + label.len());
    let Kind::Link(children, actual, href, _) = &nodes[0].kind else {
        panic!("link")
    };
    assert_eq!(actual, &label);
    assert_eq!(href, &label);
    assert!(matches!(&children[0].kind, Kind::Text(text) if text.decoded == label));
    assert_eq!(&document.source[nodes[1].range.clone()], source_tail);
    assert!(matches!(&nodes[1].kind, Kind::Text(text) if text.decoded == decoded_tail));
}

#[test]
fn list_lazy_continuations_stop_after_nonparagraph_blocks() {
    for first in ["# h", "    code", "```", "***"] {
        let source = format!("- {first}\noutside");
        let document = parse(source.clone());
        assert_eq!(document.nodes.len(), 2, "{source:?}");
        assert!(matches!(&document.nodes[0].kind, Kind::List(..)));
        assert_eq!(
            &document.source[document.nodes[0].range.clone()],
            format!("- {first}\n")
        );
        assert_eq!(&document.source[document.nodes[1].range.clone()], "outside");
    }
}

#[test]
fn list_tabs_align_continuations_without_losing_source_ranges() {
    let document = parse("- a\n\n\tb".into());
    assert_eq!(document.nodes.len(), 1);
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    let paragraphs: Vec<_> = children
        .iter()
        .filter(|node| matches!(node.kind, Kind::Paragraph(_)))
        .collect();
    assert_eq!(paragraphs.len(), 2);
    assert_eq!(&document.source[paragraphs[1].range.clone()], "\tb");
}

#[test]
fn tabbed_list_table_operands_keep_original_source_coordinates() {
    let source = "- | a |\n  | - |\n  | x\thttps://x.y/a&amp;b tail |";
    let document = parse(source.into());

    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("item")
    };
    let Kind::Table(table) = &children[0].kind else {
        panic!("table")
    };
    let cell = &table.rows[0][0];
    let context = maestro_markdown::InlineContext::new(&document);
    let scope = context.scope(&cell.range, false).unwrap();
    assert_eq!(scope.operands.len(), 1);
    let (range, node) = &scope.operands[0];
    assert_eq!(&scope.text()[range.clone()], "https://x.y/a&amp;b");
    assert_eq!(&document.source[node.range.clone()], "https://x.y/a&amp;b");
}

#[test]
fn list_continuations_separate_measurement_spelling_and_history() {
    for (source, expected, outside) in [
        ("- a\n`b\tc`", "b\tc", false),
        ("- a\n\t`b\tc`", "b    c", false),
        ("- a\n`b\tc`\ntail", "b\tc", true),
    ] {
        for options in [
            maestro_markdown::MarkedOptions::Terminal,
            maestro_markdown::MarkedOptions::Browser { escape_html: false },
            maestro_markdown::MarkedOptions::Browser { escape_html: true },
            maestro_markdown::MarkedOptions::Export,
        ] {
            let document = maestro_markdown::parse_with_options(source.into(), options);
            let Kind::List(_, items) = &document.nodes[0].kind else {
                panic!("list")
            };
            let Kind::Item(children) = &items[0].kind else {
                panic!("item")
            };
            let Kind::Inline(children) = &children[0].kind else {
                panic!("inline")
            };
            let code = children
                .iter()
                .find(|n| matches!(n.kind, Kind::Code(_)))
                .unwrap();
            assert_eq!(&source[code.range.clone()], "`b\tc`");
            assert!(
                matches!(&code.kind, Kind::Code(text) if text == expected),
                "{source:?} {options:?}"
            );
            assert_eq!(
                document.nodes.len(),
                if outside { 2 } else { 1 },
                "{source:?}"
            );
            if outside {
                assert_eq!(&source[document.nodes[1].range.clone()], "tail");
            }
        }
    }
}
