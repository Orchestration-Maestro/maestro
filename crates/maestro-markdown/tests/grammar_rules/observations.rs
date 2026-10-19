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
    /// Terminal, raw browser, escaped browser and export observations.
    targets: [Value; 4],
}

/// Require the complete selected family without deduplicating repeated observations.
pub(super) fn check(group: &str, count: usize) {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/grammar_observations.json")).unwrap();
    let mut seen = 0;
    let mut failures = Vec::new();
    for row in rows.into_iter().filter(|row| row.group == group) {
        seen += 1;
        for (options, expected) in [
            MarkedOptions::Terminal,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
            MarkedOptions::Export,
        ]
        .into_iter()
        .zip(row.targets)
        {
            let mut document =
                parse_with_options(maestro_text::normalize_to_lf(&row.input), options);
            autolinks(&mut document);
            // The export grammar's recording keeps only the authored image label.
            let actual = nodes(&document.nodes, options != MarkedOptions::Export);
            if actual != expected {
                failures.push(format!("{} {options:?}: {actual} != {expected}", row.id));
            }
        }
    }
    assert_eq!(seen, count);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Compare native semantic fields while coalescing adjacent display text only.
fn nodes(input: &[Node], image_children: bool) -> Value {
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
