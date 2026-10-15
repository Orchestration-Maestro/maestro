//! Actual toolkit frames and fixture replay for the offline header.
#![cfg(test)]
use base64::Engine as _;
use egui::{Context, Event, FullOutput, Pos2, RawInput, Rect, Shape, Vec2};
use maestro_theme::BrandMode;
use maestro_web::export_html::template::{
    HeaderAction, HeaderState, HeaderView, SessionDocument, load_document, render_header,
};
use serde_json::Value;
use std::io;

/// Load raw data through the exported document reader.
pub fn document(data: &Value) -> SessionDocument {
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(data).unwrap());
    load_document(&encoded, "?leafId=a", None).unwrap()
}
/// Persistent real toolkit input and most recent painted output.
#[derive(Default)]
pub struct Frame {
    /// Context retaining real selection and pointer state.
    pub ctx: Context,
    /// Most recent output.
    pub output: FullOutput,
    /// Monotonic input clock.
    pub time: f64,
}
impl Frame {
    /// Draw one frame with a supplied date effect.
    pub fn draw(
        &mut self,
        doc: &SessionDocument,
        state: &mut HeaderState,
        events: Vec<Event>,
        date: &dyn Fn(&Value) -> io::Result<String>,
    ) -> io::Result<Vec<HeaderAction>> {
        let pack = maestro_theme::load_brand_pack(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
            &maestro_theme::theme::NativeThemeOperations,
        )
        .unwrap();
        let p = pack.presentation(BrandMode::Dark).unwrap();
        self.time += 0.05;
        let mut result = Ok(Vec::new());
        self.output = self.ctx.run_ui(
            RawInput {
                time: Some(self.time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1800., 4000.))),
                events,
                ..Default::default()
            },
            |ui| {
                result = render_header(
                    ui,
                    doc,
                    state,
                    HeaderView {
                        presentation: &p,
                        localize_date: date,
                    },
                );
            },
        );
        self.output.textures_delta.clear();
        result
    }
    /// Draw a successful frame whose absent date must not be read.
    pub fn plain(
        &mut self,
        doc: &SessionDocument,
        state: &mut HeaderState,
        events: Vec<Event>,
    ) -> Vec<HeaderAction> {
        self.draw(doc, state, events, &|_| unreachable!()).unwrap()
    }
    /// Painted text and its actual rectangle, preserving output order and duplicates.
    pub fn labels(&self) -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        for shape in &self.output.shapes {
            collect(&shape.shape, &mut out);
        }
        out
    }
    /// Complete painted text sequence.
    pub fn texts(&self) -> Vec<String> {
        self.labels().into_iter().map(|(s, _)| s).collect()
    }
    /// Position a pointer within a painted label.
    pub fn at(&self, text: &str) -> Pos2 {
        self.labels()
            .iter()
            .find(|(s, _)| s == text)
            .unwrap_or_else(|| panic!("missing label {text:?}"))
            .1
            .center()
    }
}
/// Traverse all painted shape groups without discarding repeated occurrences.
fn collect(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((
            t.galley.job.text.clone(),
            Rect::from_min_size(t.pos, t.galley.size()),
        )),
        Shape::Vec(shapes) => {
            for s in shapes {
                collect(s, out);
            }
        }
        _ => {}
    }
}
/// Controlled primary pointer transition.
pub fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
/// A complete plain click.
pub fn click(pos: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        pointer(pos, true),
        pointer(pos, false),
    ]
}

/// Recorded header output, decoded strictly rather than dropping fixture fields.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Literal identity label.
    pub title: String,
    /// Shortcut help.
    pub help: String,
    /// Ordered controls.
    pub buttons: Vec<Button>,
    /// Ordered metadata labels and values.
    pub info: Vec<[String; 2]>,
    /// Optional prompt.
    pub prompt: Option<Prompt>,
    /// Ordered tool details.
    pub tools: Vec<Tool>,
}
/// One control's text and tooltip.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    /// Visible text.
    pub text: String,
    /// Tooltip text.
    pub title: String,
}
/// Recorded prompt views.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prompt {
    /// Whether clicks expand this prompt.
    pub expandable: bool,
    /// Section heading.
    pub heading: String,
    /// Collapsed content.
    pub preview: Option<String>,
    /// Collapsed hint.
    pub hint: Option<String>,
    /// Complete content.
    pub full: String,
}
/// Recorded tool schema view.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Literal name.
    pub name: String,
    /// Literal description.
    pub description: String,
    /// Whether it exposes parameters.
    pub expandable: bool,
    /// Collapsed hint.
    pub hint: Option<String>,
    /// Complete ordered parameter views.
    pub parameters: Vec<Parameter>,
}
/// One schema parameter.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    /// Literal key.
    pub name: String,
    /// Displayed type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Exact required membership.
    pub required: bool,
    /// Optional literal description.
    pub description: Option<String>,
}
/// Corpus envelope.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// All independent inputs.
    rows: Vec<Case>,
}
/// Recorded operation input.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Raw export.
    pub data: Option<Value>,
    /// Optional aggregate snapshot override.
    pub stats: Option<Snapshot>,
    /// Numeric/date operation selector.
    pub operation: Option<String>,
    /// Numeric operand, including named nonfinite values.
    pub count: Option<Value>,
    /// Native date operand.
    pub timestamp: Option<Value>,
    /// Whether this row owns the multi-control sequence.
    pub interaction: Option<bool>,
}
/// One source-derived observation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Case {
    /// Unique fixture label.
    pub name: String,
    /// Raw operation operands.
    pub input: Input,
    /// Success or invalid-data branch.
    pub branch: String,
    /// Header or numeric/native-date result.
    pub output: Option<Value>,
    /// Expected failure category.
    pub error_kind: Option<String>,
    /// Actual recorded expansion observations.
    pub interactions: Option<Vec<Value>>,
}
/// Select each fixture input for its named behavior owner.
pub fn cases(predicate: impl Fn(&str) -> bool) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/session_header.json")).unwrap();
    corpus
        .rows
        .into_iter()
        .filter(|c| predicate(&c.name))
        .collect()
}
/// Read the exact header expectation.
pub fn expected(c: &Case) -> Expected {
    serde_json::from_value(c.output.clone().unwrap()).unwrap()
}
/// Decode a named numeric operand without dropping negative zero.
pub fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        Some(value) => value.parse().unwrap(),
        None => v.as_f64().unwrap(),
    }
}
/// Caller-supplied statistics override with strict fixture fields.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Snapshot {
    /// Ordered model identities.
    models: Option<Vec<String>>,
    /// User count.
    user_messages: Option<usize>,
    /// Assistant count.
    assistant_messages: Option<usize>,
    /// Result count.
    tool_results: Option<usize>,
    /// Custom count.
    custom_messages: Option<usize>,
    /// Compaction count.
    compactions: Option<usize>,
    /// Branch count.
    branch_summaries: Option<usize>,
    /// Invocation count.
    tool_calls: Option<usize>,
    /// Reported token operands.
    tokens: Option<Components>,
    /// Reported cost operands.
    cost: Option<Components>,
}
/// The independently supplied numeric components.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Components {
    /// Input operand.
    input: Option<f64>,
    /// Output operand.
    output: Option<f64>,
    /// Read operand.
    cache_read: Option<f64>,
    /// Write operand.
    cache_write: Option<f64>,
    /// Supplied total, deliberately not used by header accounting.
    total: Option<f64>,
}
/// Apply only the supplied prepared-statistics fields.
pub fn prepared(c: &Case) -> SessionDocument {
    let mut doc = document(c.input.data.as_ref().unwrap());
    let Some(stats) = &c.input.stats else {
        return doc;
    };
    let s = &mut doc.global_stats;
    for (slot, value) in [
        (&mut s.user_messages, stats.user_messages),
        (&mut s.assistant_messages, stats.assistant_messages),
        (&mut s.tool_results, stats.tool_results),
        (&mut s.custom_messages, stats.custom_messages),
        (&mut s.compactions, stats.compactions),
        (&mut s.branch_summaries, stats.branch_summaries),
        (&mut s.tool_calls, stats.tool_calls),
    ] {
        if let Some(n) = value {
            *slot = n;
        }
    }
    if let Some(models) = &stats.models {
        s.models.clone_from(models);
    }
    if let Some(tokens) = &stats.tokens {
        components(
            tokens,
            [
                &mut s.tokens.input,
                &mut s.tokens.output,
                &mut s.tokens.cache_read,
                &mut s.tokens.cache_write,
                &mut s.tokens.total_tokens,
            ],
        );
    }
    if let Some(cost) = &stats.cost {
        components(
            cost,
            [
                &mut s.tokens.cost.input,
                &mut s.tokens.cost.output,
                &mut s.tokens.cost.cache_read,
                &mut s.tokens.cost.cache_write,
                &mut s.tokens.cost.total,
            ],
        );
    }
    doc
}
/// Set each provided operand without treating absence as zero.
fn components(values: &Components, slots: [&mut f64; 5]) {
    for (slot, value) in slots.into_iter().zip([
        values.input,
        values.output,
        values.cache_read,
        values.cache_write,
        values.total,
    ]) {
        if let Some(value) = value {
            *slot = value;
        }
    }
}
/// Assert the exact metadata sequence, without synthesizing labels from presence.
pub fn metadata(frame: &Frame, e: &Expected) {
    let mut wanted = vec![e.title.clone(), e.help.clone()];
    wanted.extend(e.buttons.iter().map(|b| b.text.clone()));
    wanted.extend(e.info.iter().flatten().cloned());
    assert_eq!(&frame.texts()[..wanted.len()], wanted);
}
/// Assert only the tool section, preserving ordering and duplicate labels.
pub fn tool_texts(frame: &Frame, tools: &[Tool], expanded: &[bool]) {
    let mut wanted = Vec::new();
    if !tools.is_empty() {
        wanted.push("Available Tools".to_owned());
    }
    for (tool, &open) in tools.iter().zip(expanded) {
        wanted.extend([tool.name.clone(), " - ".into(), tool.description.clone()]);
        assert_eq!(tool.expandable, tool.hint.is_some());
        if tool.expandable {
            wanted.push(if open {
                "[hide parameters]".into()
            } else {
                tool.hint.clone().unwrap()
            });
        }
        if !open {
            continue;
        }
        for param in &tool.parameters {
            let required = if param.required {
                "required"
            } else {
                "optional"
            };
            wanted.extend([param.name.clone(), param.kind.clone(), required.into()]);
            wanted.extend(param.description.iter().cloned());
        }
    }
    let all = frame.texts();
    let start = all
        .iter()
        .position(|s| s == "Available Tools")
        .unwrap_or(all.len());
    assert_eq!(&all[start..], wanted);
}
/// Exercise collapsed and expanded output, with independent positional widgets.
pub fn replay_tools(c: &Case) {
    let doc = prepared(c);
    let e = expected(c);
    let mut frame = Frame::default();
    let mut state = HeaderState::default();
    frame
        .draw(&doc, &mut state, vec![], &|_| unreachable!())
        .unwrap();
    let mut expanded = vec![false; e.tools.len()];
    tool_texts(&frame, &e.tools, &expanded);
    for (position, tool) in e.tools.iter().enumerate() {
        if !tool.expandable {
            continue;
        }
        let collapsed_position = expanded.iter().take(position).filter(|&&b| b).count();
        let hint = tool.hint.as_ref().unwrap();
        let pos = frame
            .labels()
            .iter()
            .filter(|(s, _)| s == hint)
            .nth(position - collapsed_position)
            .unwrap()
            .1
            .center();
        frame
            .draw(&doc, &mut state, click(pos), &|_| unreachable!())
            .unwrap();
        frame
            .draw(&doc, &mut state, vec![], &|_| unreachable!())
            .unwrap();
        expanded[position] = true;
        tool_texts(&frame, &e.tools, &expanded);
    }
}
/// Actual themed frame assertions.
pub mod theme;
