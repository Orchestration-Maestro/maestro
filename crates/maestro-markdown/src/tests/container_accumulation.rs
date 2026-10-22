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
    let chained = [32, 64, 128]
        .map(|n| measured_containers(format!("> - a\n{}e", "      p\n> [s]: /y\n".repeat(n))));
    let visits = chained.map(|work| work.list_lines);
    // The shared rule sets compile at most twice per cold key, however often its quote resumes.
    let prepared = chained.map(|work| work.list_rules);
    assert!(prepared[0] <= 2, "{prepared:?}");
    assert_eq!(&prepared[1..], &[0, 0], "{prepared:?}");
    let copied = chained.map(|work| work.source_bytes + work.joined_bytes);
    let built_in = chained.map(|work| {
        work.block_recognizer_bytes
            + work.definition_bytes
            + work.paragraph_bytes
            + work.table_bytes
            + work.fence_bytes
            + work.code_bytes
            + work.html_bytes
            + work.framing_bytes
            + work.source_views
            + work.projections
    });
    eprintln!(
        "chained list visits: {visits:?}; copied bytes: {copied:?}; built-in work: {built_in:?}"
    );
    let admission_linear = visits.windows(2).all(|pair| pair[1] <= 3 * pair[0]);
    let copying_linear = copied.windows(2).all(|pair| pair[1] <= 3 * pair[0]);
    let built_in_linear = built_in.windows(2).all(|pair| pair[1] <= 3 * pair[0]);
    assert!(
        admission_linear && copying_linear && built_in_linear,
        "admission linear: {admission_linear}; copying linear: {copying_linear}; built-in linear: {built_in_linear}; {chained:?}"
    );
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
}

/// Item lookahead rules prepared while parsing one list of one-line items.
fn item_rule_preparations(lines: usize) -> usize {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let source = "- item\n".repeat(lines);
    assert_eq!(parse(source).nodes.len(), 1);
    super::WORK.with_borrow(|work| work.list_rules)
}

#[test]
fn item_rule_preparation_does_not_grow_with_list_length() {
    let counts = [16, 256, 4096].map(item_rule_preparations);
    assert!(counts[0] <= 2, "{counts:?}");
    assert_eq!(&counts[1..], &[0, 0], "{counts:?}");
}

#[test]
fn resumed_list_admits_completed_items_once() {
    let chained = |completed: usize, n: usize| {
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let source =
            "> - x\n".repeat(completed) + "> - a\n" + &"      p\n> [s]: /y\n".repeat(n) + "e";
        let document = parse(source);
        let Kind::Quote(children, _) = &document.nodes[0].kind else {
            panic!("quote")
        };
        let Kind::List(_, items) = &children[0].kind else {
            panic!("list")
        };
        assert_eq!(items.len(), completed + 1);
        super::WORK.with_borrow(super::container_frames::container_work)
    };
    // Work the completed prefix adds beyond the same resumed item without it.
    let added = [64, 128, 256].map(|n| chained(n, n) - chained(0, n));
    eprintln!("work added by completed items: {added:?}");
    assert!(
        added.windows(2).all(|pair| pair[1] <= 3 * pair[0]),
        "{added:?}"
    );
}

#[test]
fn long_item_steps_copy_only_admitted_operands() {
    let mut superlinear = Vec::new();
    for (unit, options) in [
        (
            "\n  p\n",
            crate::MarkedOptions::Browser { escape_html: true },
        ),
        ("\n  > q\n", crate::MarkedOptions::Terminal),
        (
            "\n  > q\n",
            crate::MarkedOptions::Browser { escape_html: true },
        ),
    ] {
        let copied = [32, 64, 128].map(|n| {
            super::WORK.with_borrow_mut(|work| *work = super::Work::default());
            let document = crate::parse_with_options(format!("- a\n{}", unit.repeat(n)), options);
            let Kind::List(_, items) = &document.nodes[0].kind else {
                panic!("list")
            };
            assert!(
                matches!(&items[..], [item] if matches!(&item.kind, Kind::Item(blocks) if blocks.len() > n))
            );
            super::WORK.with_borrow(|work| work.source_bytes + work.joined_bytes)
        });
        eprintln!("{unit:?} {options:?}: copied bytes {copied:?}");
        if copied.windows(2).any(|pair| pair[1] > 3 * pair[0]) {
            superlinear.push(format!("{unit:?} {options:?}: {copied:?}"));
        }
    }
    assert!(superlinear.is_empty(), "{superlinear:#?}");
}

/// Compare actual wrong-whole-suffix selection with the borrowed-window control.
fn measured_suffixes(unit: &str, n: usize, copy: bool) -> super::Work {
    let source = crate::source::ScopeSource::identity(&unit.repeat(n));
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    for start in (0..source.text.len()).step_by(unit.len()) {
        if copy {
            let _piece = crate::source::ScopeSource::identity(&source.text[start..]);
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
        document.nodes[0].authored.to_owned_string(),
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

#[test]
fn merged_code_scope_ends_at_consumed_source_prefix() {
    let source = "> q\n    code\n>       \n> y";
    let document = parse(source.into());
    let Kind::Quote(children, _) = &document.nodes[0].kind else {
        panic!("quote")
    };
    let paragraph = &children[0];
    let scope = &document.scopes[&(paragraph.range.start, paragraph.range.end, false)];
    assert_eq!(&*scope.text, "q\ncode");
    assert_eq!(&document.source[paragraph.range.clone()], "q\n    code");
    assert_eq!(scope.native(&(2..6)), Some(8..12));
    assert!(matches!(children[1].kind, Kind::Gap));
    assert_eq!(paragraph_text(&children[2]).as_deref(), Some("y"));
}
