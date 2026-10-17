#![cfg(test)]

use maestro_markdown::{Alignment, Kind, LinkType, parse};

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
    assert_eq!(metadata.id, "ref");
    assert_eq!(metadata.link_type, LinkType::Reference);
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
fn parser_retains_container_source_extents() {
    let document = parse(
        "> $$x\n> y$$\n\n- a \\*b\\* `opaque`\n  tail\n\n| a | b |\n| - | - |\n| \\| | &amp; |\n"
            .to_owned(),
    );
    let Kind::Quote(quote, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    assert_eq!(quote[0].authored, "$$x\ny$$\n");
    assert_eq!(&document.source[quote[0].range.clone()], "$$x\n> y$$\n");
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
fn parser_orders_native_structure_before_mapped_links() {
    let mut document = parse("$x$ https://example.org/$q$ &amp; www.example.org. \\* https://other.org\n\n| a |\n| - |\n| www.example.org/\\| |\n".to_owned());
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    assert!(
        !children
            .iter()
            .any(|node| matches!(node.kind, Kind::Link(..)))
    );
    maestro_markdown::autolinks(&mut document);
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
    let mut document =
        parse("https://example.test/a $x$ www.example.test \\(y\\) a@example.test".into());
    maestro_markdown::autolinks(&mut document);
    let Kind::Paragraph(children) = &document.nodes[0].kind else {
        panic!("paragraph")
    };
    for node in children {
        if let Kind::Link(..) = &node.kind {
            assert_eq!(&document.source[node.range.clone()], node.authored);
        }
    }
}
