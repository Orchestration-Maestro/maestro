//! Semantic field observations from the three caller grammars.
use maestro_markdown::{
    Alignment, Cell, Kind, MarkedOptions, Node, Table, autolinks, parse_with_options,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// One independently recorded input with targets in caller order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Stable observation identity.
    id: String,
    /// Grammar family that owns the assertion.
    group: String,
    /// Original authored input.
    input: String,
    /// Observations in the selected caller order.
    targets: Vec<Value>,
    /// Recorded joined authored spellings at block inline scopes.
    ///
    /// Absent for families recorded without metadata; a null target is deliberately
    /// excluded. Absence and null targets match those of `link_labels`.
    inline_authored: Option<[Option<Vec<String>>; 4]>,
    /// Recorded link labels, absent and excluded exactly where `inline_authored` is.
    link_labels: Option<[Option<Vec<String>>; 4]>,
    /// Corrected raw envelopes, when this family asserts source consumption.
    raw_quotes: Option<[Vec<String>; 4]>,
}

/// Require the complete selected family without deduplicating repeated observations.
pub(super) fn check(group: &str, count: usize, fixture: &str) {
    check_with_options(
        group,
        count,
        fixture,
        &[
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
            MarkedOptions::Export,
        ],
    );
}

/// Compare recorded observations for the grammars that own the selected rule.
pub(super) fn check_with_options(
    group: &str,
    count: usize,
    fixture: &str,
    options: &[MarkedOptions],
) {
    let rows: Vec<Row> = serde_json::from_str(fixture).unwrap();
    let mut seen = 0;
    let mut failures = Vec::new();
    for row in rows.into_iter().filter(|row| row.group == group) {
        seen += 1;
        assert_eq!(row.targets.len(), options.len(), "{}", row.id);
        for (target, (options, expected)) in options.iter().copied().zip(&row.targets).enumerate() {
            let mut document =
                parse_with_options(maestro_text::normalize_to_lf(&row.input), options);
            if let Some(raw) = &row.raw_quotes {
                check_raw(&document, &raw[target], &row.id);
            }
            autolinks(&mut document);
            if let Some(difference) = metadata_difference(&row, target, &document.nodes) {
                failures.push(format!("{} {options:?}: {difference}", row.id));
            }
            // The export grammar's recording keeps only the authored image label.
            let actual = nodes(&document.nodes, options != MarkedOptions::Export);
            if actual != *expected {
                failures.push(format!("{} {options:?}: {actual} != {expected}", row.id));
            }
        }
    }
    assert_eq!(seen, count);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Describe a mismatch only for recorded, non-excluded metadata targets.
fn metadata_difference(row: &Row, target: usize, nodes: &[Node]) -> Option<String> {
    let expected = (
        recorded(row.inline_authored.as_ref(), target),
        recorded(row.link_labels.as_ref(), target),
    );
    let (Some(authored), Some(expected_labels)) = expected else {
        assert_eq!(
            expected,
            (None, None),
            "{}: metadata recorded apart",
            row.id
        );
        return None;
    };
    let (mut actual_authored, mut labels) = (Vec::new(), Vec::new());
    metadata(nodes, &mut actual_authored, &mut labels);
    (actual_authored != *authored || labels != *expected_labels).then(|| {
        format!("metadata {actual_authored:?} / {labels:?} != {authored:?} / {expected_labels:?}")
    })
}

/// One field's recording for a target: `None` when absent from the row or excluded there.
fn recorded(field: Option<&[Option<Vec<String>>; 4]>, target: usize) -> Option<&Vec<String>> {
    field?[target].as_ref()
}

/// Collect authored inline scopes and link labels.
fn metadata(input: &[Node], authored: &mut Vec<String>, labels: &mut Vec<String>) {
    for node in input {
        let (scope, children): (bool, Vec<&[Node]>) = match &node.kind {
            Kind::Paragraph(children) | Kind::Inline(children) | Kind::Heading(_, children, _) => {
                (true, vec![children.as_slice()])
            }
            Kind::Table(table) => (
                true,
                table
                    .header
                    .iter()
                    .chain(table.rows.iter().flatten())
                    .map(|cell| cell.children.as_slice())
                    .collect(),
            ),
            Kind::Link(children, label, ..) => {
                labels.push(label.clone());
                (false, vec![children.as_slice()])
            }
            Kind::Item(children)
            | Kind::List(_, children)
            | Kind::Quote(children, _)
            | Kind::Strong(children)
            | Kind::Emphasis(children)
            | Kind::Strike(children) => (false, vec![children.as_slice()]),
            Kind::Image(_, meta) => (false, vec![meta.children.as_slice()]),
            _ => (false, Vec::new()),
        };
        if scope {
            authored.extend(
                children
                    .iter()
                    .map(|nodes| nodes.iter().map(|child| child.authored.as_str()).collect()),
            );
        }
        for nodes in children {
            metadata(nodes, authored, labels);
        }
    }
}

/// Compare raw quote envelopes separately from semantic inline projections.
fn check_raw(document: &maestro_markdown::Document, expected: &[String], id: &str) {
    let quotes: Vec<_> = document
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, Kind::Quote(..)))
        .collect();
    assert_eq!(
        quotes.iter().map(|node| &node.authored).collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>(),
        "{id}"
    );
    for quote in quotes {
        assert_eq!(quote.authored, document.source[quote.range.clone()], "{id}");
    }
}

/// Compare native semantic fields while coalescing adjacent display text only.
pub(super) fn nodes(input: &[Node], image_children: bool) -> Value {
    let mut output: Vec<Value> = Vec::new();
    for node in input {
        let value = observe(&node.kind, image_children);
        if value[0] == "text" && output.last().is_some_and(|last| last[0] == "text") {
            let last = output.last_mut().unwrap();
            last[1] = json!(format!(
                "{}{}",
                last[1].as_str().unwrap(),
                value[1].as_str().unwrap()
            ));
        } else {
            output.push(value);
        }
    }
    json!(output)
}

/// Observe one block or inline value without comparing renderer implementation details.
///
/// `image_children` compares image label children, which only some recordings keep.
fn observe(kind: &Kind, image_children: bool) -> Value {
    match kind {
        Kind::Paragraph(children) => json!(["paragraph", nodes(children, image_children)]),
        Kind::Inline(children) => json!(["inline", nodes(children, image_children)]),
        Kind::Item(children) => json!(["item", nodes(children, image_children)]),
        Kind::Heading(level, children, _) => {
            json!(["heading", level, nodes(children, image_children)])
        }
        Kind::List(start, children) => json!(["list", start, nodes(children, image_children)]),
        Kind::Quote(children, _) => json!(["quote", nodes(children, image_children)]),
        Kind::Text(text) => json!(["text", text.decoded]),
        Kind::Code(text) => json!(["code", text]),
        Kind::CodeBlock(text, info) => json!(["code-block", text, info]),
        Kind::Html(text) => json!(["html", text]),
        Kind::HtmlBlock(text) => json!(["html-block", text]),
        Kind::Strong(children) => json!(["strong", nodes(children, image_children)]),
        Kind::Emphasis(children) => json!(["emphasis", nodes(children, image_children)]),
        Kind::Strike(children) => json!(["strike", nodes(children, image_children)]),
        Kind::Link(children, _, href, meta) => {
            json!(["link", href, meta.title, nodes(children, image_children)])
        }
        Kind::Image(alt, meta) if image_children => json!([
            "image",
            alt,
            meta.destination,
            meta.title,
            nodes(&meta.children, image_children)
        ]),
        Kind::Image(alt, meta) => json!(["image", alt, meta.destination, meta.title]),
        Kind::TaskListMarker(checked) => json!(["task", checked]),
        Kind::Gap => json!(["gap"]),
        Kind::Rule => json!(["rule"]),
        Kind::Break => json!(["break"]),
        Kind::CustomBlock(kind, text) => json!(["custom-block", kind, text]),
        Kind::Atom(index) => json!(["atom", index]),
        Kind::Table(table) => observe_table(table, image_children),
    }
}

/// Observe column alignments and each cell's children in authored order.
fn observe_table(table: &Table, image_children: bool) -> Value {
    let cells = |cells: &[Cell]| {
        cells
            .iter()
            .map(|cell| nodes(&cell.children, image_children))
            .collect::<Vec<_>>()
    };
    json!([
        "table",
        table
            .alignments
            .iter()
            .map(|align| match align {
                Alignment::None => "none",
                Alignment::Left => "left",
                Alignment::Center => "center",
                Alignment::Right => "right",
            })
            .collect::<Vec<_>>(),
        cells(&table.header),
        table.rows.iter().map(|row| cells(row)).collect::<Vec<_>>()
    ])
}
