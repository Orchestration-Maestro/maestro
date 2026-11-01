//! Strict recorded replay for custom-message and skill-invocation widgets.
use crate::palette;
use crate::registry::REGISTRY;
use crate::registry::Restore;
use maestro_app::{
    CustomMessage, ImageContent, MessageRenderOptions, MessageRenderer, MessageRendererCall,
    ParsedSkillBlock, Presence, TextContent, UserBlock, UserContent, create_event_bus,
};
use maestro_chat::{CustomMessageComponent, SkillInvocationMessageComponent};
use maestro_theme::{LiveTheme, ThemeColor, ThemeState, paint};
use maestro_tui::{
    Component, KeybindingDefinition, KeybindingKeys, KeybindingsManager, MarkdownTheme,
    TerminalImage, Text, get_keybindings, set_keybindings, tui::ComponentHandle,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashSet,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// One recorded widget invocation with its independent expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning test name.
    test: String,
    /// Recorded constructor and operation inputs.
    input: Input,
    /// Recorded rows, frames or callback observations.
    expected: Expected,
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<Expected>,
}

/// Recorded constructor and operation inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Widget kind.
    kind: String,
    /// Render width, defaulting to the recorded eighty columns.
    width: Option<usize>,
    /// String or ordered text/image content.
    content: Option<Content>,
    /// Custom message classification.
    custom_type: Option<String>,
    /// Whether the outer transcript should display the message.
    display: Option<bool>,
    /// Skill name.
    name: Option<String>,
    /// Requested expansion state.
    expanded: Option<bool>,
    /// Configured expansion keys.
    keys: Option<Vec<String>>,
    /// One creation outcome.
    mode: Option<String>,
    /// One creation outcome per rebuild.
    modes: Option<Vec<String>>,
    /// Ordered operations after construction.
    #[serde(default)]
    ops: Vec<Operation>,
}

/// Recorded message content.
#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    /// Plain authored text.
    Text(String),
    /// Ordered text and image parts.
    Parts(Vec<Part>),
}

/// One recorded content part.
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum Part {
    /// Authored text.
    #[serde(rename = "text")]
    Text {
        /// Part text.
        text: String,
    },
    /// Base64 image data the default display ignores.
    #[serde(rename = "image")]
    Image {
        /// Encoded data.
        data: String,
        /// Declared media type.
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

/// One recorded operation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Operation {
    /// Expansion request.
    Expand(bool),
    /// Named operation.
    Named(Named),
}

/// Named operations recorded after construction.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Named {
    /// Rebuilds from current theme and registry values.
    Invalidate,
    /// Requests the current expansion state again.
    Same,
    /// Publishes the second recording palette.
    Theme,
}

/// Recorded expectation, either complete rows or a multi-frame record.
enum Expected {
    /// One complete frame.
    Rows(Vec<String>),
    /// Frames and callback observations.
    Detail(Detail),
}
impl<'de> Deserialize<'de> for Expected {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let raw = Value::deserialize(decoder)?;
        match &raw {
            Value::Array(_) => serde_json::from_value(raw)
                .map(Self::Rows)
                .map_err(D::Error::custom),
            Value::Object(fields) => {
                if let Some(name) = fields
                    .keys()
                    .find(|name| !EXPECTATION_FIELDS.contains(&name.as_str()))
                {
                    return Err(D::Error::custom(format!(
                        "unrecorded expectation field: {name}"
                    )));
                }
                serde_json::from_value(raw)
                    .map(Self::Detail)
                    .map_err(D::Error::custom)
            }
            _ => Err(D::Error::custom("expectation is complete rows or a record")),
        }
    }
}

/// Record fields the multi-frame expectation admits.
const EXPECTATION_FIELDS: &[&str] = &["rows", "frames", "trace", "invalidations", "oldChildren"];

/// Multi-frame expectation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    /// One complete frame.
    rows: Option<Vec<String>>,
    /// Frames in operation order.
    frames: Option<Vec<Vec<String>>>,
    /// Renderer observations in call order.
    trace: Option<Vec<Trace>>,
    /// Child invalidations the renderer observed.
    invalidations: Option<usize>,
    /// Child count of the retained old array.
    old_children: Option<usize>,
}

/// One renderer observation.
#[derive(Clone, Debug, PartialEq)]
enum Trace {
    /// Expansion flag only.
    Flag(bool),
    /// Complete creation outcome.
    Seen(Box<Seen>),
}
impl<'de> Deserialize<'de> for Trace {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        match Value::deserialize(decoder)? {
            Value::Bool(value) => Ok(Self::Flag(value)),
            value @ Value::Object(_) => serde_json::from_value(value)
                .map(|seen| Self::Seen(Box::new(seen)))
                .map_err(D::Error::custom),
            _ => Err(D::Error::custom("trace is a flag or a recorded outcome")),
        }
    }
}

/// What a recording renderer observed.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Seen {
    /// Classification the callback received.
    custom_type: String,
    /// Content the callback received.
    content: Value,
    /// Details the callback received.
    details: Value,
    /// Display flag the callback received.
    display: bool,
    /// Timestamp the callback received.
    timestamp: f64,
    /// Expansion flag the callback received.
    expanded: bool,
    /// Foreground proof sampled through the supplied theme.
    theme_color: String,
}

/// Replays one recorded group, keeping the key registry admitted and restored.
pub fn group(name: &str) {
    let _serial = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    for case in cases().iter().filter(|case| case.test == name) {
        bind(vec!["ctrl+o".into()]);
        match name {
            "custom_renderer_outcomes_select_only_one_body" => outcomes(case),
            "custom_expansion_skips_equal_values_and_invalidation_rebuilds" => expansion(case),
            "custom_rebuild_replaces_renderers_and_preserves_outer_edits" => rebuild(case),
            "custom_theme_refresh_samples_background_before_foreground_rebuild" => {
                custom_theme_refresh(case);
            }
            "skill_hints_read_live_configured_keys_on_each_rebuild" => hints(case),
            "skill_repeated_expansion_and_invalidation_refresh_current_theme" => {
                skill_theme_refresh(case);
            }
            "skill_supplied_markdown_callbacks_and_child_aliases_survive" => skill_supplied(case),
            _ => constant(case),
        }
    }
}

/// Reads the complete committed record, rejecting duplicate inputs and stale groups.
fn cases() -> Vec<Case> {
    let raw: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/custom_and_skill_messages.json"))
            .expect("recorded cases parse");
    let inputs: HashSet<_> = raw.iter().map(|row| row["input"].to_string()).collect();
    assert_eq!(inputs.len(), raw.len(), "recorded inputs are unique");
    let cases: Vec<Case> = raw
        .into_iter()
        .map(|row| {
            let mut case: Case = serde_json::from_value(row).expect("recorded case is strict");
            if cfg!(target_os = "macos") {
                case.expected = case.expected_macos.take().unwrap_or(case.expected);
            }
            case
        })
        .collect();
    let groups: HashSet<&str> = cases.iter().map(|case| case.test.as_str()).collect();
    assert_eq!(cases.len(), 48);
    assert_eq!(groups, GROUPS.iter().copied().collect());
    cases
}

/// Every recorded group, one per public test.
const GROUPS: &[&str] = &[
    "custom_default_strings_keep_markdown_and_panel_rows",
    "custom_text_parts_keep_order_duplicates_and_empty_boundaries",
    "custom_labels_are_literal_bold_and_do_not_filter_display",
    "message_layout_reuses_padding_and_wrapping",
    "custom_renderer_outcomes_select_only_one_body",
    "custom_expansion_skips_equal_values_and_invalidation_rebuilds",
    "custom_rebuild_replaces_renderers_and_preserves_outer_edits",
    "custom_theme_refresh_samples_background_before_foreground_rebuild",
    "custom_supplied_markdown_callbacks_are_retained",
    "skill_collapsed_and_expanded_rows_keep_their_own_spacing",
    "skill_expanded_body_keeps_markdown_and_ignores_other_fields",
    "skill_hints_read_live_configured_keys_on_each_rebuild",
    "skill_repeated_expansion_and_invalidation_refresh_current_theme",
    "skill_supplied_markdown_callbacks_and_child_aliases_survive",
];

/// Publishes the named recording palette and keeps its state for a later refresh.
fn published(variant: &str) -> (ThemeState, LiveTheme) {
    crate::published::published(&palette::palette(variant))
}

/// Installs the recorded expand keys.
fn bind(keys: Vec<String>) {
    set_keybindings(KeybindingsManager::new(
        vec![(
            "app.tools.expand".into(),
            KeybindingDefinition {
                default_keys: KeybindingKeys::Multiple(keys),
                description: None,
            },
        )],
        vec![],
    ));
}

/// Uses the recorded palette with the real Markdown callbacks and highlighter.
fn markdown(live: LiveTheme, supplied: bool) -> Rc<MarkdownTheme> {
    let mut theme = maestro_theme::get_markdown_theme(
        live,
        Rc::new(maestro_theme::SyntectHighlighter::new().expect("highlighter")),
        true,
    );
    if supplied {
        theme.heading = Box::new(|text| format!("CUSTOM[{text}]"));
    }
    Rc::new(theme)
}

/// Shared terminal without image capabilities, as recorded.
fn terminal() -> TerminalImage {
    TerminalImage::new(|_| None, || 1)
}

/// Prepares the recorded custom message.
fn message(input: &Input) -> CustomMessage<Value> {
    let content = match input.content.as_ref() {
        Some(Content::Text(text)) => UserContent::Text(text.clone()),
        Some(Content::Parts(parts)) => UserContent::Blocks(
            parts
                .iter()
                .map(|part| match part {
                    Part::Text { text } => UserBlock::Text(TextContent::new(text.clone())),
                    Part::Image { data, mime_type } => UserBlock::Image(ImageContent {
                        data: data.clone(),
                        mime_type: mime_type.clone(),
                    }),
                })
                .collect(),
        ),
        None => UserContent::Text("body".into()),
    };
    CustomMessage {
        custom_type: input.custom_type.clone().unwrap_or_else(|| "note".into()),
        content,
        display: input.display.unwrap_or(true),
        details: Presence::Present(serde_json::json!({"answer": 7})),
        timestamp: 1.5,
    }
}

/// Builds the recorded skill block with its unread fields present.
fn skill_block(input: &Input) -> ParsedSkillBlock {
    ParsedSkillBlock {
        name: input.name.clone().unwrap_or_else(|| "build".into()),
        location: "/skills/build/SKILL.md".into(),
        content: match input.content.as_ref() {
            Some(Content::Text(text)) => text.clone(),
            _ => "Do **work**.\n\nSecond paragraph.".into(),
        },
        user_message: Some("separate user text".into()),
    }
}

/// Binds an inline native renderer to a fresh empty bus.
fn projected(
    renderer: impl Fn(&CustomMessage<Value>, MessageRenderOptions, &LiveTheme) -> Creation
    + Send
    + Sync
    + 'static,
) -> MessageRendererCall<Value> {
    let renderer: MessageRenderer<Value> = Arc::new(move |message, options, theme, _| {
        renderer(&message, options, &theme).map_err(Into::into)
    });
    let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
    MessageRendererCall::new(renderer, (*bus).clone())
}

/// One recorded creation outcome.
type Creation = Result<Option<ComponentHandle>, String>;

/// Renders one constructed widget and compares the complete rows.
fn constant(case: &Case) {
    let Expected::Rows(expected) = &case.expected else {
        panic!("{} expects complete rows", case.test);
    };
    let live = published("a").1;
    let theme = markdown(live.clone(), case.test.contains("supplied_markdown"));
    let width = case.input.width.unwrap_or(80);
    let rows = if case.input.kind == "custom" {
        CustomMessageComponent::new(
            Arc::new(message(&case.input)),
            None::<MessageRendererCall>,
            live,
            theme,
            terminal(),
        )
        .render(width)
    } else {
        let skill =
            SkillInvocationMessageComponent::new(skill_block(&case.input), live, theme, terminal());
        skill.set_expanded(case.input.expanded.unwrap_or(false));
        skill.render(width)
    };
    assert_eq!(&rows, expected, "{}", case.test);
}

/// Records what a creation renderer receives and selects one body.
fn outcomes(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    let mode = case.input.mode.clone().expect("recorded mode");
    let renderer =
        move |message: &CustomMessage<Value>, options: MessageRenderOptions, theme: &LiveTheme| {
            recorded
                .lock()
                .expect("trace")
                .push(Trace::Seen(Box::new(Seen {
                    custom_type: message.custom_type.clone(),
                    content: serde_json::to_value(&message.content).expect("content"),
                    details: serde_json::to_value(&message.details).expect("details"),
                    display: message.display,
                    timestamp: message.timestamp,
                    expanded: options.expanded,
                    theme_color: paint(
                        theme.get().ok().as_deref(),
                        &ThemeColor::CustomMessageText,
                        "sample",
                    ),
                })));
            match mode.as_str() {
                "throw" => Err("creation failed".into()),
                "component" => Ok(Some(
                    Rc::new(Text::new("CUSTOM".into(), 0, 0, None)) as ComponentHandle
                )),
                _ => Ok(None),
            }
        };
    let live = published("a").1;
    let widget = CustomMessageComponent::new(
        Arc::new(message(&case.input)),
        Some(projected(renderer)),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    assert_eq!(&widget.render(80), detail.rows.as_ref().expect("rows"));
    let trace: Vec<_> = seen.lock().expect("trace").drain(..).collect();
    assert_eq!(Some(trace), detail.trace.clone());
}

/// Records expansion flags and the child invalidations the renderer observed.
fn expansion(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let flags = Arc::new(Mutex::new(Vec::new()));
    let invalidations = Arc::new(AtomicUsize::new(0));
    let (recorded, counted) = (Arc::clone(&flags), Arc::clone(&invalidations));
    let renderer = move |_message: &CustomMessage<Value>,
                         options: MessageRenderOptions,
                         _theme: &LiveTheme| {
        recorded
            .lock()
            .expect("trace")
            .push(Trace::Flag(options.expanded));
        Ok(Some(Rc::new(CountingText {
            text: Text::new(format!("custom {}", options.expanded), 0, 0, None),
            invalidations: Arc::clone(&counted),
        }) as ComponentHandle))
    };
    let (state, live) = published("a");
    let widget = CustomMessageComponent::new(
        Arc::new(message(&case.input)),
        Some(projected(renderer)),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    assert_eq!(
        &replay(&widget, &state, &case.input.ops),
        detail.frames.as_ref().expect("frames")
    );
    let trace: Vec<_> = flags.lock().expect("trace").drain(..).collect();
    assert_eq!(Some(trace), detail.trace.clone());
    assert_eq!(
        Some(invalidations.load(Ordering::SeqCst)),
        detail.invalidations,
        "the replaced child observes one invalidation"
    );
}

/// Performs the recorded operations and keeps every rendered frame.
fn replay(widget: &impl Replay, state: &ThemeState, ops: &[Operation]) -> Vec<Vec<String>> {
    let mut frames = vec![widget.render(80)];
    for operation in ops {
        match operation {
            Operation::Expand(value) => widget.expand(*value),
            Operation::Named(Named::Invalidate) => widget.invalidate(),
            Operation::Named(Named::Same) => widget.expand(false),
            Operation::Named(Named::Theme) => {
                state
                    .set_theme_instance(palette::palette("b"))
                    .expect("republished theme");
            }
        }
        frames.push(widget.render(80));
    }
    frames
}

/// The expansion operation each recorded widget kind exposes.
trait Replay: Component {
    /// Requests an expansion state.
    fn expand(&self, expanded: bool);
}
impl Replay for SkillInvocationMessageComponent {
    fn expand(&self, expanded: bool) {
        self.set_expanded(expanded);
    }
}
impl<T: 'static, E: maestro_app::ExtensionExecution> Replay for CustomMessageComponent<T, E> {
    fn expand(&self, expanded: bool) {
        self.set_expanded(expanded);
    }
}

/// Rebuilds across recorded creation outcomes while an outer alias stays stable.
fn rebuild(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let modes = Arc::new(Mutex::new(
        case.input
            .modes
            .clone()
            .expect("recorded modes")
            .into_iter(),
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let (queue, counted) = (Arc::clone(&modes), Arc::clone(&calls));
    let renderer = move |_message: &CustomMessage<Value>,
                         _options: MessageRenderOptions,
                         _theme: &LiveTheme| {
        let mode = queue.lock().expect("modes").next().expect("outcome");
        let call = counted.fetch_add(1, Ordering::SeqCst) + 1;
        match mode.as_str() {
            "throw" => Err("failure".into()),
            "component" => Ok(Some(
                Rc::new(Text::new(format!("custom {call}"), 0, 0, None)) as ComponentHandle,
            )),
            _ => Ok(None),
        }
    };
    let live = published("a").1;
    let widget = CustomMessageComponent::new(
        Arc::new(message(&case.input)),
        Some(projected(renderer)),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    let captured = widget.children();
    widget.add_child(Rc::new(Text::new("tail".into(), 0, 0, None)));
    let mut frames = vec![widget.render(80)];
    for _ in 0..3 {
        widget.invalidate();
        frames.push(widget.render(80));
    }
    assert_eq!(&frames, detail.frames.as_ref().expect("frames"));
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(
        Rc::ptr_eq(&widget.children(), &captured),
        "outer array identity"
    );
}

/// Samples the live background before and after a foreground rebuild.
fn custom_theme_refresh(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let (state, live) = published("a");
    let widget = CustomMessageComponent::new(
        Arc::new(message(&case.input)),
        None::<MessageRendererCall>,
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    let frames = replay(&widget, &state, &case.input.ops);
    assert_eq!(&frames, detail.frames.as_ref().expect("frames"));
}

/// Reads configured keys on each rebuild, never before one.
fn hints(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let live = published("a").1;
    let widget = SkillInvocationMessageComponent::new(
        skill_block(&case.input),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    let mut frames = vec![widget.render(80)];
    bind(case.input.keys.clone().expect("recorded keys"));
    frames.push(widget.render(80));
    widget.set_expanded(false);
    frames.push(widget.render(80));
    bind(vec!["ctrl+y".into()]);
    widget.invalidate();
    frames.push(widget.render(80));
    assert_eq!(&frames, detail.frames.as_ref().expect("frames"));
}

/// Rebuilds equal expansion values against a republished theme.
fn skill_theme_refresh(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let (state, live) = published("a");
    let widget = SkillInvocationMessageComponent::new(
        skill_block(&case.input),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    let frames = replay(&widget, &state, &case.input.ops);
    assert_eq!(&frames, detail.frames.as_ref().expect("frames"));
}

/// Supplies a distinguishable heading callback and retains the original array.
fn skill_supplied(case: &Case) {
    let Expected::Detail(detail) = &case.expected else {
        panic!("{} expects a record", case.test);
    };
    let live = published("a").1;
    let widget = SkillInvocationMessageComponent::new(
        skill_block(&case.input),
        live.clone(),
        markdown(live, true),
        terminal(),
    );
    let old = widget.children();
    widget.add_child(Rc::new(Text::new("extra".into(), 0, 0, None)));
    let mut frames = vec![widget.render(80)];
    widget.set_expanded(true);
    frames.push(widget.render(80));
    assert!(
        !Rc::ptr_eq(&widget.children(), &old),
        "the rebuild installs a current array"
    );
    assert_eq!(Some(old.borrow().len()), detail.old_children);
    assert_eq!(&frames, detail.frames.as_ref().expect("frames"));
}

/// A child that counts invalidations without a clock.
struct CountingText {
    /// Rendered text, which owns the recorded padding.
    text: Text,
    /// Invalidations observed by this child.
    invalidations: Arc<AtomicUsize>,
}
impl Component for CountingText {
    fn render(&self, width: usize) -> Vec<String> {
        self.text.render(width)
    }
    fn invalidate(&self) {
        self.invalidations.fetch_add(1, Ordering::SeqCst);
        self.text.invalidate();
    }
}

/// Selects only text parts, in order, joined by newlines.
pub fn traversal() {
    let cases: Vec<Traversal> =
        serde_json::from_str(include_str!("../fixtures/custom_message_traversal.json"))
            .expect("recorded traversal cases parse");
    assert_eq!(cases.len(), 3);
    for case in cases {
        let blocks: Vec<UserBlock> = (0..case.n)
            .map(|index| {
                if index % 2 == 0 {
                    UserBlock::Text(TextContent::new("x".into()))
                } else {
                    UserBlock::Image(ImageContent {
                        data: "x".into(),
                        mime_type: "image/png".into(),
                    })
                }
            })
            .collect();
        assert_eq!(UserBlock::text_output(&blocks), case.expected_text);
    }
}

/// One recorded text-selection case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Traversal {
    /// Part count.
    n: usize,
    /// Selected text.
    expected_text: String,
}

/// Creation failures select the default body while later child failures propagate.
pub fn later_render_errors() {
    let (_, live) = published("a");
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let renderer = move |_message: &CustomMessage<Value>,
                         _options: MessageRenderOptions,
                         _theme: &LiveTheme| {
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(Some(
            Rc::new(FailingChild("later render failed")) as ComponentHandle
        ))
    };
    let widget = CustomMessageComponent::new(
        Arc::new(message(&recorded_input())),
        Some(projected(renderer)),
        live.clone(),
        markdown(live.clone(), false),
        terminal(),
    );
    let failure = widget
        .try_render(80)
        .expect_err("the child failure propagates");
    assert_eq!(failure.to_string(), "later render failed");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "no second creation");
    let rows = widget.render(80);
    assert!(rows.iter().any(|row| row.contains("failing child")));
    assert!(
        !rows.iter().any(|row| row.contains("[note]")),
        "no default panel after a chosen component"
    );
    let control = CustomMessageComponent::new(
        Arc::new(message(&recorded_input())),
        None::<MessageRendererCall>,
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    control.add_child(Rc::new(FailingChild("control failed")));
    let control_failure = control
        .try_render(80)
        .expect_err("an unrelated failing child fails the walk");
    assert_eq!(control_failure.to_string(), "control failed");
}

/// The recorded custom input without a case record.
fn recorded_input() -> Input {
    Input {
        kind: "custom".into(),
        width: None,
        content: None,
        custom_type: None,
        display: None,
        name: None,
        expanded: None,
        keys: None,
        mode: None,
        modes: None,
        ops: Vec::new(),
    }
}

/// Replaced children and retained owners are released without a clock.
pub fn released_handles() {
    let (_, live) = published("a");
    let drops = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let (counted_drops, counted_calls) = (Arc::clone(&drops), Arc::clone(&calls));
    let callback: MessageRenderer<Value> = Arc::new(move |_message, _options, _theme, _| {
        counted_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Rc::new(TrackedChild {
            drops: Arc::clone(&counted_drops),
        }) as ComponentHandle))
    });
    let callback_handle = Arc::downgrade(&callback);
    let message = Arc::new(message(&recorded_input()));
    let message_handle = Arc::downgrade(&message);
    let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
    let widget = CustomMessageComponent::new(
        Arc::clone(&message),
        Some(MessageRendererCall::new(callback, (*bus).clone())),
        live.clone(),
        markdown(live, false),
        terminal(),
    );
    let alias = widget.children();
    widget.set_expanded(false);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "an unchanged flag keeps the child"
    );
    widget.set_expanded(true);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "the replaced child is released"
    );
    assert_eq!(alias.borrow().len(), 2, "the retained array stays usable");
    drop(alias);
    drop(message);
    drop(widget);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        2,
        "the final child is released"
    );
    assert!(
        message_handle.upgrade().is_none(),
        "the message is released"
    );
    assert!(
        callback_handle.upgrade().is_none(),
        "the callback is released"
    );
}

/// A child that reports its own release.
struct TrackedChild {
    /// Releases shared with the test.
    drops: Arc<AtomicUsize>,
}
impl Component for TrackedChild {
    fn render(&self, _width: usize) -> Vec<String> {
        vec!["tracked child".into()]
    }
}
impl Drop for TrackedChild {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/// A child whose later render fails while its ordinary render still works.
struct FailingChild(&'static str);
impl Component for FailingChild {
    fn render(&self, _width: usize) -> Vec<String> {
        vec!["failing child".into()]
    }
    fn try_render(&self, _width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Err(self.0.into())
    }
}
