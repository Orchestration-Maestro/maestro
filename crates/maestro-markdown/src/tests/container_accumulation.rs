//! Shared-tail and source-consumption observations at recursive grammar boundaries.
use crate::{Kind, parse};

#[test]
fn recursive_accumulator_keeps_tail_state_and_consumption() {
    let mut failures = Vec::new();
    for (source, expected) in [
        ("> q\n    code", "q\ncode"),
        ("> q\n[r]: /x\n> y", "q\n[r]: /x\ny"),
    ] {
        let document = parse(source.into());
        let Kind::Quote(children, _) = &document.nodes[0].kind else {
            panic!("quote")
        };
        let paragraph = children.first().and_then(paragraph_text);
        if paragraph.as_deref() != Some(expected) || children.len() != 1 {
            failures.push(format!(
                "shared tail {source:?}: {paragraph:?}, children={}",
                children.len()
            ));
        }
    }
    let document = parse(">".into());
    if !matches!(&document.nodes[0].kind, Kind::Quote(children, _) if children.is_empty()) {
        failures.push("empty quote lost recognized marker".into());
    }
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse("> - a\n      p\n> [s]: /y\ne".into());
    let (end, previous) = super::WORK.with_borrow(|work| (work.reentry_end, work.reentry_previous));
    assert_eq!((end, previous), (13, 23));
    assert_eq!(document.nodes[0].range.end, document.source.len());
    assert_eq!(document.definitions.len(), 1);
    assert_eq!(document.definitions["s"].destination, "/y");
    duplicate_definition_keeps_visible_tail();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Observe only literal paragraph operands; links remain distinguishable.
fn paragraph_text(node: &crate::Node) -> Option<String> {
    let Kind::Paragraph(children) = &node.kind else {
        return None;
    };
    Some(
        children
            .iter()
            .filter_map(|node| match &node.kind {
                Kind::Text(text) => Some(text.decoded.as_str()),
                _ => None,
            })
            .collect(),
    )
}

#[test]
fn merged_unicode_scopes_keep_native_character_boundaries() {
    let document = parse("> é\n    😀\n> https://frame.test/é".into());
    let scope = document.scopes.get(&(2, 37, false)).unwrap();
    assert_eq!(scope.local(&(4..13)), Some(2..7));
    assert_eq!(scope.local(&(9..13)), Some(3..7));
    assert_eq!(scope.native(&(2..3)), Some(4..4));
    assert_eq!(scope.native(&(3..7)), Some(9..13));
    let mut document = document;
    crate::autolinks(&mut document);
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
    assert_eq!(&document.source[link.range.clone()], "https://frame.test/é");
}

/// Actual grammar and mapping work for one repeated container workload.
fn measured_containers(source: String) -> super::Work {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse(source);
    assert!(!document.nodes.is_empty());
    super::WORK.with_borrow(|work| *work)
}

#[test]
fn container_mapping_matches_the_reference_work_shape() {
    for unit in [
        "> a\n> b\n\n",
        "- a\n  b\n\n",
        "> - a\n      p\n> [s]: /y\ne\n\n",
    ] {
        let counts = [32, 64].map(|n| measured_containers(unit.repeat(n)));
        let work =
            |w: super::Work| w.source_bytes + w.joined_bytes + w.source_views + w.projections;
        assert!(work(counts[1]) < 3 * work(counts[0]), "{counts:?}");
        assert!(
            counts[1].quote_lines <= 2 * counts[0].quote_lines,
            "{counts:?}"
        );
        assert!(
            counts[1].list_rules <= 2 * counts[0].list_rules,
            "{counts:?}"
        );
        let copied = [32, 64].map(|n| measured_suffixes(unit, n, true));
        let borrowed = [32, 64].map(|n| measured_suffixes(unit, n, false));
        assert!(copied[1].source_bytes > 3 * copied[0].source_bytes);
        assert_eq!(borrowed[1].source_bytes, 0);
        assert_eq!(borrowed[1].source_views, 2 * borrowed[0].source_views);
        eprintln!("container {unit:?}: {counts:?}");
    }
    let counts = [32, 64].map(|n| {
        measured_containers("> - a\n".to_owned() + &"      p\n> [s]: /y\n".repeat(n) + "e")
    });
    assert!(
        counts[1].list_lines > 3 * counts[0].list_lines,
        "{counts:?}"
    );
    eprintln!("grammar-owned chained reentry: {counts:?}");
}

/// Compare actual wrong-whole-suffix selection with the borrowed-window control.
fn measured_suffixes(unit: &str, n: usize, copy: bool) -> super::Work {
    let source = crate::source::ScopeSource::identity(&unit.repeat(n));
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    for start in (0..source.text.len()).step_by(unit.len()) {
        if copy {
            let _piece = source.select(std::iter::once(start..source.text.len()));
        } else {
            let _view = source.view(start..source.text.len());
        }
    }
    super::WORK.with_borrow(|work| *work)
}

/// Independently recorded reference maps for one complete input.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Definitions {
    /// Stable case identity for failures.
    id: String,
    /// Original complete input.
    input: String,
    /// Destination and title for each first-wins identity in caller order.
    targets: [std::collections::BTreeMap<String, (String, String)>; 4],
}

#[test]
fn container_definitions_keep_caller_specific_first_wins_maps() {
    let rows: Vec<Definitions> = serde_json::from_str(include_str!(
        "../../tests/fixtures/container_definitions.json"
    ))
    .unwrap();
    for row in rows {
        for (options, expected) in [
            crate::MarkedOptions::Terminal,
            crate::MarkedOptions::Browser { escape_html: false },
            crate::MarkedOptions::Browser { escape_html: true },
            crate::MarkedOptions::Export,
        ]
        .into_iter()
        .zip(row.targets)
        {
            let document =
                crate::parse_with_options(maestro_text::normalize_to_lf(&row.input), options);
            let actual: std::collections::BTreeMap<_, _> = document
                .definitions
                .into_iter()
                .map(|(id, definition)| (id, (definition.destination, definition.title)))
                .collect();
            assert_eq!(actual, expected, "{} {options:?}", row.id);
        }
    }
}

/// A duplicate definition does not replace the browser's effective visible tail.
fn duplicate_definition_keeps_visible_tail() {
    let document = crate::parse_with_options(
        "> [s]: /first\n> - a\n      p\n> [s]: /y\ne".into(),
        crate::MarkedOptions::Browser { escape_html: false },
    );
    assert_eq!(document.definitions["s"].destination, "/first");
    let Kind::Quote(children, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    assert!(matches!(children[1].kind, Kind::Gap));
}

#[test]
fn raw_list_space_keeps_the_consumed_source_spelling() {
    let document = parse("- a ".into());
    assert_eq!(
        document.nodes[0].authored,
        document.source[document.nodes[0].range.clone()]
    );
}

#[test]
fn subsequent_empty_marker_uses_the_item_framing_rule() {
    let document = parse("- a\n- \n".into());
    let Kind::List(_, items) = &document.nodes[0].kind else {
        panic!("list")
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[1].kind, Kind::Item(children) if children.is_empty()));
}

#[test]
fn item_framing_keeps_first_line_history_and_blank_admission() {
    for (input, count) in [
        ("- a\né    code\noutside", 1),
        ("- a\n😀    code\noutside", 2),
        ("-    a\noutside", 2),
        ("- \tcode\noutside", 2),
        ("-\n\n\n- b", 3),
    ] {
        assert_eq!(parse(input.into()).nodes.len(), count, "{input:?}");
    }
}
