//! Transcript witnesses through the interactive mode's retained containers and shortcuts.
use super::super::replay::aborted_notice;
use super::{Setup, World, isolated};
use crate::BranchSummaryMessageComponent;
use crate::CustomMessageComponent;
use crate::interactive_mode::startup::ExpandableText;
use maestro_app::{
    AgentMessage, AgentToolResult, BranchSummaryMessage, CompactionOptions, CustomMessage,
    ExtensionContext, ExtensionFactory, Local, MessageRenderOptions, NativeAgentMessage, Presence,
    ToolRenderResult, ToolRenderResultLike, UserContent, is_whitespace_scalar,
};
use maestro_theme::{ColorMode, ColorValue, LiveTheme, Theme, ThemeBg, ThemeColor, ThemeOptions};
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Component, Container, EditorComponent, MarkdownTheme, Spacer, Text};
use maestro_tui::{ImageProtocol, TerminalCapabilities};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use serde_json::json;
use std::cell::Cell;
use std::rc::Weak;
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc};

/// The recorded palette and every recorded transcript case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_transcript.json");

/// The recorded corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Foreground and background palette indexes the recording used.
    palette: Palette,
    /// The recorded cases.
    cases: Vec<Case>,
}

/// The palette's colour indexes by role name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Palette {
    /// Foreground roles.
    foreground: BTreeMap<String, u8>,
    /// Background roles.
    background: BTreeMap<String, u8>,
}

/// One recorded case, read by its witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Value,
    /// The recorded observation.
    expected: Value,
}

/// The transcript rows and the effects of one operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transcript {
    /// The transcript rows at width 100.
    rows: Vec<String>,
    /// Unresolved tool calls, in registration order.
    pending: Vec<String>,
    /// Editor history entries, oldest first.
    history: Vec<String>,
    /// Frames requested.
    renders: usize,
    /// The startup help's rows at width 100.
    #[serde(default)]
    header: Option<Vec<String>>,
    /// Custom renderer invocations.
    #[serde(default)]
    calls: Option<usize>,
    /// What the registered renderers observed, in call order.
    #[serde(default)]
    events: Option<Vec<Value>>,
    /// The saved hidden-thinking preference.
    #[serde(default)]
    hidden: Option<bool>,
}

/// The recorded corpus.
fn corpus() -> Corpus {
    serde_json::from_str(FIXTURES).unwrap()
}

/// The recorded cases of `test`, in order, read as its operation and observation records.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    let cases: Vec<_> = corpus()
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty(), "missing group {test}");
    cases
}

/// The theme painting the recorded palette indexes.
fn palette() -> Rc<Theme> {
    let palette = corpus().palette;
    let foreground = palette
        .foreground
        .into_iter()
        .map(|(role, index)| (ThemeColor::Named(role), ColorValue::Index(index)));
    let background = palette
        .background
        .into_iter()
        .map(|(role, index)| (ThemeBg::Named(role), ColorValue::Index(index)));
    Rc::new(
        Theme::new(
            foreground,
            background,
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}

/// A mode in `directory` with images hidden, publishing the recorded palette.
pub(super) fn recorded(directory: &Path, setup: Setup) -> World {
    let configure = setup.configure;
    let world = Setup {
        configure: Box::new(move |settings| {
            settings.set_show_images(false);
            configure(settings);
        }),
        ..setup
    }
    .build(directory);
    world.themes.set_theme_instance(palette()).unwrap();
    world
}

/// The Markdown theme the transcript rows of `world` use.
fn markdown(world: &World) -> Rc<MarkdownTheme> {
    Rc::new(maestro_theme::get_markdown_theme(
        world.themes.theme(),
        world.mode.tool_environment.highlighter.clone(),
        true,
    ))
}

/// A displayed custom message of `custom_type`.
fn custom(custom_type: &str) -> Rc<CustomMessage> {
    Rc::new(CustomMessage {
        custom_type: custom_type.to_owned(),
        content: UserContent::Text(String::new()),
        display: true,
        details: Presence::Missing,
        timestamp: 0.0,
    })
}

/// An expansion over the startup help and a summary row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expansion {
    /// The operation name.
    op: String,
    /// Whether an installed custom header was active; only built-in cases are recorded here.
    custom: bool,
    /// The expansion applied.
    expanded: bool,
}

#[test]
fn transcript_expansion_reaches_active_header_and_expandable_rows() {
    let test = "transcript_expansion_reaches_active_header_and_expandable_rows";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Expansion, Transcript>(test) {
            let Expansion {
                op,
                custom,
                expanded,
            } = input;
            assert_eq!((op.as_str(), custom), ("expand", false));
            let world = recorded(directory, Setup::default());
            let header = Rc::new(
                ExpandableText::new(
                    Box::new(|| Ok("HEADER_SHORT".to_owned())),
                    Box::new(|| Ok("HEADER_FULL".to_owned())),
                    false,
                    0,
                    0,
                )
                .unwrap(),
            );
            world
                .mode
                .built_in_header
                .replace(Some(header.clone() as Rc<dyn Component>));
            let chat = &world.mode.chat_container;
            chat.add_child(Rc::new(Spacer::new(1)));
            chat.add_child(Rc::new(BranchSummaryMessageComponent::new(
                BranchSummaryMessage {
                    summary: "BRANCH_BODY".into(),
                    from_id: "from".into(),
                    timestamp: 0.0,
                },
                world.themes.theme(),
                markdown(&world),
                world.mode.tool_environment.terminal_image.clone(),
            )));
            chat.add_child(Rc::new(Text::new("PLAIN".into(), 0, 0, None)));
            frames(&world);
            world.mode.set_tools_expanded(expanded).unwrap();
            assert_eq!(world.mode.tool_output_expanded.get(), expanded);
            assert_eq!(Some(header.render(100)), expected.header);
            observe(&world, &expected, 0);
        }
    });
}

/// Replays the stored `record` alone and checks that expanding the transcript changes what
/// its row paints.
fn assert_expansion_changes(test: &str, record: Value) {
    isolated(test, async |directory| {
        let world = recorded(directory, Setup::default());
        frames(&world);
        replay(&world, vec![record]);
        let collapsed = world.mode.chat_container.render(100);
        world.mode.set_tools_expanded(true).unwrap();
        assert_ne!(
            world.mode.chat_container.render(100),
            collapsed,
            "expanding leaves the row as it was"
        );
    });
}

#[test]
fn transcript_expansion_reaches_shell_runs() {
    let output = (1..=30)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_expansion_changes(
        "transcript_expansion_reaches_shell_runs",
        json!({"role": "bashExecution", "command": "seq 30", "output": output,
            "exitCode": 0, "cancelled": false, "truncated": false, "timestamp": 0}),
    );
}

#[test]
fn transcript_expansion_reaches_compaction_summaries() {
    assert_expansion_changes(
        "transcript_expansion_reaches_compaction_summaries",
        json!({"role": "compactionSummary", "summary": "COMPACTION_BODY",
            "tokensBefore": 1234, "timestamp": 0}),
    );
}

#[test]
fn transcript_expansion_reaches_skill_rows() {
    assert_expansion_changes(
        "transcript_expansion_reaches_skill_rows",
        user_record("<skill name=\"review\" location=\"/work/SKILL.md\">\nSKILL_BODY\n</skill>"),
    );
}

/// The transcript slot the walk renderers edit, filled once the mode exists.
type Slot = Rc<RefCell<Option<Rc<Container>>>>;

/// Applies `edit` to the transcript in `slot` when `name` is the first row.
fn edited(name: &str, edit: &str, slot: &Slot, rows: &RefCell<Vec<ComponentHandle>>) {
    let chat = slot.borrow().clone().unwrap();
    let rows = rows.borrow().clone();
    match (name, edit) {
        ("first", "append") => chat.add_child(rows[2].clone()),
        ("first", "remove_next") => chat.remove_child(&rows[1]),
        ("first", "replace_array") => {
            chat.set_children(Rc::new(RefCell::new(vec![rows[2].clone()])));
        }
        ("first", "clear") => chat.clear(),
        _ => {}
    }
}

/// An extension whose renderers for `first`, `second` and `extra` record each expansion
/// in `seen`, and whose `first` renderer applies `edit` to the transcript in `slot`.
fn walking(
    edit: String,
    slot: Slot,
    rows: Rc<RefCell<Vec<ComponentHandle>>>,
    seen: Rc<RefCell<Vec<String>>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        for name in ["first", "second", "extra"] {
            let walk = (edit.clone(), slot.clone(), rows.clone(), seen.clone());
            register_walker(&api, name, walk);
        }
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The edit, transcript slot, created rows and expansion record one walk renderer shares.
type Walker = (
    String,
    Slot,
    Rc<RefCell<Vec<ComponentHandle>>>,
    Rc<RefCell<Vec<String>>>,
);

/// Registers the renderer of `name`, recording its expansions and applying the edit.
fn register_walker(
    api: &maestro_app::ExtensionAPI<Local>,
    name: &'static str,
    (edit, slot, rows, seen): Walker,
) {
    api.register_message_renderer::<Value>(
        name.into(),
        Rc::new(move |_, options: MessageRenderOptions, _: LiveTheme, _| {
            if options.expanded {
                seen.borrow_mut().push(name.into());
                edited(name, &edit, &slot, &rows);
            }
            Ok(Some(text(name)))
        }),
    )
    .unwrap();
}

/// Asserts the transcript and every row a walk made are released.
fn assert_released((transcript, rows): &(Weak<Container>, Vec<Weak<dyn Component>>)) {
    assert!(transcript.upgrade().is_none(), "the transcript is released");
    assert!(
        rows.iter().all(|row| row.upgrade().is_none()),
        "the rows are released"
    );
}

/// A transcript edit the first renderer makes while it is expanded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Walk {
    /// The operation name.
    op: String,
    /// `append`, `remove_next`, `replace_array` or `clear`.
    edit: String,
}

/// The expansion order and the transcript left afterwards.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Walked {
    /// Rows expanded, in order.
    seen: Vec<String>,
    /// The transcript's rows afterwards.
    children: Vec<String>,
    /// Frames requested.
    renders: usize,
}

#[test]
fn transcript_expansion_keeps_the_captured_child_array_live() {
    let test = "transcript_expansion_keeps_the_captured_child_array_live";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Walk, Walked>(test) {
            assert_eq!(input.op, "walk");
            let (slot, rows, seen) = (Slot::default(), Rc::default(), Rc::default());
            let edit = input.edit;
            let world = recorded(
                directory,
                Setup {
                    extension: Some(walking(
                        edit,
                        slot.clone(),
                        Rc::clone(&rows),
                        Rc::clone(&seen),
                    )),
                    ..Setup::default()
                },
            );
            let chat = Rc::clone(&world.mode.chat_container);
            slot.replace(Some(Rc::clone(&chat)));
            let runner = world.host.session().extension_runner();
            let made: Vec<ComponentHandle> = ["first", "second", "extra"]
                .into_iter()
                .map(|name| {
                    Rc::new(CustomMessageComponent::<Value, Local>::new(
                        custom(name),
                        runner.get_message_renderer_call(name),
                        world.themes.theme(),
                        markdown(&world),
                        world.mode.tool_environment.terminal_image.clone(),
                    )) as ComponentHandle
                })
                .collect();
            rows.replace(made.clone());
            chat.add_child(made[0].clone());
            chat.add_child(made[1].clone());
            world.mode.set_tools_expanded(true).unwrap();
            assert_eq!(*seen.borrow(), expected.seen);
            // Each custom row adds its own leading spacer row around the renderer's child.
            let left: Vec<_> = chat
                .render(100)
                .iter()
                .map(|row| row.trim_end().to_owned())
                .filter(|row| !row.is_empty())
                .collect();
            assert_eq!(left, expected.children);
            assert_eq!(world.runtime.pending(), expected.renders);
            let released = (
                Rc::downgrade(&chat),
                made.iter().map(Rc::downgrade).collect(),
            );
            rows.borrow_mut().clear();
            slot.replace(None);
            drop((made, chat, runner, world));
            assert_released(&released);
        }
    });
}

#[test]
fn transcript_registered_toggle_actions_release_the_mode() {
    isolated(
        "transcript_registered_toggle_actions_release_the_mode",
        async |directory| {
            let World {
                mode,
                terminal,
                runtime,
                themes,
                ..
            } = recorded(directory, Setup::default());
            let world_theme = |_: &Rc<crate::InteractiveMode>| Rc::clone(&themes);
            mode.init().await.unwrap();
            runtime.settle().unwrap();
            terminal.send_input("\x0f");
            assert!(mode.tool_output_expanded.get());
            assert_eq!(runtime.pending(), 1);
            terminal.send_input("\x0f");
            assert!(!mode.tool_output_expanded.get());
            runtime.settle().unwrap();
            let empty = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
            world_theme(&mode)
                .set_theme_instance(Rc::new(empty))
                .unwrap();
            terminal.send_input("\x14");
            assert!(mode.hide_thinking_block.get());
            let errors = runtime.drain_futures();
            let [error] = <[_; 1]>::try_from(errors).ok().unwrap();
            assert!(
                error.downcast_ref::<maestro_theme::ThemeError>().is_some(),
                "{error}"
            );
            let editor = Rc::clone(&mode.editor);
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            runtime.settle().ok();
            for key in ["\x0f", "\x14"] {
                maestro_tui::tui::InputHandler::handle_input(editor.as_ref(), key);
            }
            assert_eq!(runtime.pending(), 0);
            assert!(runtime.drain_futures().is_empty());
        },
    );
}

/// The frames `world` has requested since the last count, which are then drawn.
pub(super) fn frames(world: &World) -> usize {
    let requested = world.runtime.pending();
    world.runtime.settle().unwrap();
    requested
}

/// The editor history, oldest first, recalled through the editor's own browsing.
pub(super) fn history(world: &World) -> Vec<String> {
    let editor = &world.mode.editor;
    let draft = editor.get_text();
    let mut recalled = Vec::new();
    loop {
        maestro_tui::tui::InputHandler::handle_input(editor.as_ref(), "\x1b[A");
        let text = editor.get_text();
        if recalled.last().unwrap_or(&draft) == &text {
            break;
        }
        recalled.push(text);
    }
    editor.set_text(&draft);
    recalled.reverse();
    recalled
}

/// The registered unresolved tool calls, in registration order.
pub(super) fn pending(world: &World) -> Vec<String> {
    world.mode.pending_tools.borrow().keys().cloned().collect()
}

/// Asserts the transcript rows, unresolved calls, editor history and frames `world` shows
/// after one recorded operation; `earlier_frames` counts frames requested before the last
/// count. The frame writer draws once for every request made before a frame runs, where the
/// recording counts each request, so one operation's requests count as one frame.
fn observe(world: &World, expected: &Transcript, earlier_frames: usize) {
    assert_eq!(world.mode.chat_container.render(100), expected.rows);
    assert_eq!(pending(world), expected.pending);
    // The editor admits the text it is handed by its own rule: trimmed and nonempty.
    let admitted: Vec<_> = expected
        .history
        .iter()
        .map(|text| text.trim_matches(is_whitespace_scalar).to_owned())
        .filter(|text| !text.is_empty())
        .collect();
    let requested = if earlier_frames == 0 {
        expected.renders.min(1)
    } else {
        expected.renders
    };
    assert_eq!(earlier_frames + frames(world), requested);
    assert_eq!(history(world), admitted);
}

/// A native application message read from its stored record.
pub(super) fn message(record: Value) -> NativeAgentMessage {
    serde_json::from_value(record).unwrap()
}

/// A user message with the recorded content.
fn user(content: &Value) -> NativeAgentMessage {
    message(serde_json::json!({"role": "user", "content": content, "timestamp": 0}))
}

/// A user message with its content and whether it joins the history.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserInput {
    /// The operation name.
    op: String,
    /// Text or ordered text and image blocks.
    content: Value,
    /// Whether the history is populated.
    history: bool,
}

#[test]
fn transcript_user_text_keeps_order_whitespace_and_history() {
    let test = "transcript_user_text_keeps_order_whitespace_and_history";
    isolated(test, async |directory| {
        for (input, expected) in cases::<UserInput, Transcript>(test) {
            assert_eq!(input.op, "user");
            let world = recorded(directory, Setup::default());
            frames(&world);
            world
                .mode
                .add_message_to_chat(&user(&input.content), input.history)
                .unwrap();
            observe(&world, &expected, 0);
        }
    });
}

/// A skill block with an optional trailing user text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillInput {
    /// The operation name.
    op: String,
    /// Text after the skill block.
    tail: String,
    /// Whether rows start expanded.
    expanded: bool,
}

#[test]
fn transcript_skill_block_and_trailing_user_keep_distinct_rows() {
    let test = "transcript_skill_block_and_trailing_user_keep_distinct_rows";
    let skill =
        "<skill name=\"build\" location=\"/skills/build/SKILL.md\">\nDo **work**.\n</skill>";
    isolated(test, async |directory| {
        for (input, expected) in cases::<SkillInput, Transcript>(test) {
            assert_eq!(input.op, "skill");
            let world = recorded(directory, Setup::default());
            frames(&world);
            world.mode.tool_output_expanded.set(input.expanded);
            let content = Value::String(format!("{skill}{}", input.tail));
            world
                .mode
                .add_message_to_chat(&user(&content), true)
                .unwrap();
            observe(&world, &expected, 0);
        }
    });
}

/// One message of an application or model role.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleInput {
    /// The operation name.
    op: String,
    /// The message role.
    role: String,
    /// Whether rows start expanded.
    expanded: bool,
}

/// The recorded message of `role`.
fn role(role: &str) -> Value {
    let usage = serde_json::json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 0, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "total": 0}});
    match role {
        "custom" => serde_json::json!({"role": role, "customType": "note",
            "content": "CUSTOM_BODY", "display": true, "timestamp": 0}),
        "branchSummary" => serde_json::json!({"role": role, "summary": "BRANCH_BODY",
            "fromId": "from", "timestamp": 0}),
        "compactionSummary" => serde_json::json!({"role": role, "summary": "COMPACTION_BODY",
            "tokensBefore": 1234, "timestamp": 0}),
        "bashExecution" => serde_json::json!({"role": role, "command": "echo controlled",
            "output": "OUTPUT", "exitCode": 0, "cancelled": false, "truncated": false,
            "timestamp": 0}),
        "assistant" => serde_json::json!({"role": role, "content": [
            {"type": "thinking", "thinking": "REASON"}, {"type": "text", "text": "ANSWER"}],
            "api": "controlled", "provider": "controlled", "model": "controlled",
            "usage": usage, "stopReason": "stop", "timestamp": 0}),
        "toolResult" => serde_json::json!({"role": role, "toolCallId": "call",
            "toolName": "unknown_tool", "content": [{"type": "text",
            "text": "HISTORICAL_RESULT"}], "isError": false, "timestamp": 0}),
        other => panic!("unrecorded role {other}"),
    }
}

#[test]
fn transcript_message_roles_use_existing_components() {
    let test = "transcript_message_roles_use_existing_components";
    isolated(test, async |directory| {
        for (input, expected) in cases::<RoleInput, Transcript>(test) {
            assert_eq!(input.op, "role");
            let world = recorded(directory, Setup::default());
            frames(&world);
            world.mode.tool_output_expanded.set(input.expanded);
            world
                .mode
                .add_message_to_chat(&message(role(&input.role)), false)
                .unwrap();
            if input.role == "toolResult" {
                assert!(world.mode.chat_container.children().borrow().is_empty());
            }
            observe(&world, &expected, 0);
        }
    });
}

/// A custom message's visibility and its renderer's outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomInput {
    /// The operation name.
    op: String,
    /// Whether the message is displayed.
    display: bool,
    /// `none`, `component`, `undefined` (no component) or `throw`.
    renderer: String,
}

/// An extension registering a `note` renderer with `outcome`, counting its calls.
fn rendering(outcome: String, calls: Rc<Cell<usize>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        if outcome != "none" {
            let (outcome, calls) = (outcome.clone(), calls.clone());
            api.register_message_renderer::<Value>(
                "note".into(),
                Rc::new(move |_, _, _: LiveTheme, _| {
                    calls.set(calls.get() + 1);
                    match outcome.as_str() {
                        "component" => Ok(Some(text("RENDERED_CUSTOM"))),
                        "throw" => Err("declined".into()),
                        _ => Ok(None),
                    }
                }),
            )
            .unwrap();
        }
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn transcript_custom_display_filters_before_renderer_lookup() {
    let test = "transcript_custom_display_filters_before_renderer_lookup";
    isolated(test, async |directory| {
        for (input, expected) in cases::<CustomInput, Transcript>(test) {
            assert_eq!(input.op, "custom");
            let calls = Rc::new(Cell::new(0));
            let setup = Setup {
                extension: Some(rendering(input.renderer, Rc::clone(&calls))),
                ..Setup::default()
            };
            let world = recorded(directory, setup);
            frames(&world);
            let record = serde_json::json!({"role": "custom", "customType": "note",
                "content": "FALLBACK", "display": input.display, "timestamp": 0});
            world
                .mode
                .add_message_to_chat(&message(record), false)
                .unwrap();
            assert_eq!(Some(calls.get()), expected.calls);
            observe(&world, &expected, 0);
        }
    });
}

/// Two statuses with an intervening message, or one status before the transcript clears.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusInput {
    /// The operation name.
    op: String,
    /// Whether a user message comes between the two statuses.
    #[serde(default)]
    separator: Option<bool>,
    /// Whether the transcript is cleared between the two statuses.
    #[serde(default)]
    cleared: Option<bool>,
}

#[test]
fn transcript_status_reuses_only_the_adjacent_owned_pair() {
    let test = "transcript_status_reuses_only_the_adjacent_owned_pair";
    isolated(test, async |directory| {
        for (input, expected) in cases::<StatusInput, Transcript>(test) {
            assert_eq!(input.op, "status");
            let world = recorded(directory, Setup::default());
            frames(&world);
            let mode = &world.mode;
            let cleared = input.cleared == Some(true);
            let (first, second) = if cleared {
                ("OLD", "FRESH")
            } else {
                ("STATUS_ONE", "STATUS_TWO")
            };
            mode.show_status(first).unwrap();
            let old = mode.last_status.borrow().clone().unwrap();
            let mut requested = frames(&world);
            if input.separator == Some(true) {
                let content = Value::String("INTERVENING".into());
                mode.add_message_to_chat(&user(&content), false).unwrap();
            }
            if cleared {
                mode.chat_container.clear();
            }
            mode.show_status(second).unwrap();
            let current = mode.last_status.borrow().clone().unwrap();
            let replaced = input.separator == Some(true) || cleared;
            assert_eq!(!Rc::ptr_eq(&old.1, &current.1), replaced);
            let children = if input.separator == Some(true) { 6 } else { 2 };
            assert_eq!(mode.chat_container.children().borrow().len(), children);
            requested += frames(&world);
            observe(&world, &expected, requested);
        }
    });
}

/// The usage record of every recorded assistant message.
fn usage() -> Value {
    json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}

/// A stored assistant record with `content`, its stop reason and an optional error.
pub(super) fn assistant(content: &Value, stop: &str, error: Option<&str>) -> Value {
    let mut record = json!({"role": "assistant", "content": content, "api": "controlled",
        "provider": "controlled", "model": "controlled", "usage": usage(),
        "stopReason": stop, "timestamp": 0});
    if let Some(error) = error {
        record["errorMessage"] = json!(error);
    }
    record
}

/// A stored tool call of `name` with the recorded object arguments.
pub(super) fn call(id: &str, name: &str) -> Value {
    json!({"type": "toolCall", "id": id, "name": name, "arguments": {"z": 1, "a": 2}})
}

/// A stored result of call `id` with one text block.
pub(super) fn result(id: &str) -> Value {
    json!({"role": "toolResult", "toolCallId": id, "toolName": "unknown_tool",
        "content": [{"type": "text", "text": "HISTORICAL_RESULT"}], "isError": false,
        "timestamp": 0})
}

/// A stored user record with text content.
pub(super) fn user_record(text: &str) -> Value {
    json!({"role": "user", "content": text, "timestamp": 0})
}

/// Replays `messages` into the transcript of `world` without other effects.
pub(super) fn replay(world: &World, messages: Vec<Value>) {
    let context = maestro_app::SessionContext {
        messages,
        thinking_level: Presence::Present("off".into()),
        model: None,
    };
    world
        .mode
        .render_session_context(
            context,
            super::super::replay::RenderSessionContextOptions::default(),
        )
        .unwrap();
}

/// Appends `messages` to the session of `world`, returning their entry ids.
pub(super) fn append(world: &World, messages: Vec<Value>) -> Vec<String> {
    let session = world.host.session();
    let mut manager = session.session_manager().borrow_mut();
    messages
        .into_iter()
        .map(|message| manager.append_message(message).unwrap())
        .collect()
}

/// A stop reason, retry count and optional error of one failed or ordinary message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopInput {
    /// The operation name.
    op: String,
    /// The stop reason.
    stop: String,
    /// The session's retry attempts.
    retry: u32,
    /// The error message, when recorded.
    error: Option<String>,
}

#[test]
fn transcript_failed_tool_calls_finish_with_the_exact_notice() {
    let test = "transcript_failed_tool_calls_finish_with_the_exact_notice";
    isolated(test, async |directory| {
        for (input, expected) in cases::<StopInput, Transcript>(test) {
            assert_eq!(input.op, "stop");
            if input.retry > 0 {
                // A live retry alone sets the attempts; the notice is checked where it is made.
                let notice = aborted_notice(input.retry);
                assert!(
                    expected
                        .rows
                        .iter()
                        .any(|row| row.contains(&format!(" {notice} ")))
                );
                assert!(expected.pending.is_empty());
                continue;
            }
            let world = recorded(directory, Setup::default());
            frames(&world);
            assert_eq!(world.host.session().retry_attempt(), 0);
            let content = json!([call("call", "unknown_tool")]);
            replay(
                &world,
                vec![assistant(&content, &input.stop, input.error.as_deref())],
            );
            if input.stop == "aborted" {
                let notice = format!(" {} ", aborted_notice(0));
                assert!(
                    world
                        .mode
                        .chat_container
                        .render(100)
                        .iter()
                        .any(|row| row.contains(&notice))
                );
            }
            observe(&world, &expected, 0);
        }
    });
}

/// One pairing of stored calls and results.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairInput {
    /// The operation name.
    op: String,
    /// `pending`, `completed`, `orphan`, `mixed`, `out_of_order` or `duplicate_id`.
    kind: String,
}

#[test]
fn transcript_replay_pairs_results_and_retains_only_unresolved_rows() {
    let test = "transcript_replay_pairs_results_and_retains_only_unresolved_rows";
    isolated(test, async |directory| {
        for (input, expected) in cases::<PairInput, Transcript>(test) {
            assert_eq!(input.op, "pair");
            let world = recorded(directory, Setup::default());
            frames(&world);
            let calls = assistant(
                &json!([call("z", "unknown_tool"), call("a", "unknown_tool")]),
                "toolUse",
                None,
            );
            let messages = match input.kind.as_str() {
                "pending" => vec![calls],
                "completed" => vec![calls, result("z"), result("a")],
                "orphan" => vec![result("orphan")],
                "mixed" => vec![calls, result("z")],
                "out_of_order" => vec![result("z"), calls],
                "duplicate_id" => vec![
                    assistant(
                        &json!([call("z", "unknown_tool"), call("z", "unknown_tool")]),
                        "toolUse",
                        None,
                    ),
                    result("z"),
                ],
                other => panic!("unrecorded pairing {other}"),
            };
            replay(&world, messages);
            observe(&world, &expected, 0);
        }
    });
}

/// The number of compaction entries before the initial replay.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialInput {
    /// The operation name.
    op: String,
    /// Compaction entries in the session.
    count: usize,
}

/// The editor's painted top border at width 20.
fn border(world: &World) -> String {
    world.mode.editor.render(20)[0].clone()
}

/// The top border painted by the recorded palette's `role` foreground.
fn painted_border(role: &str) -> String {
    let theme = palette();
    let segment = theme.fg(&ThemeColor::Named(role.to_owned()), "─").unwrap();
    segment.repeat(20)
}

/// The recorded palette with every foreground index moved, so each role paints another colour.
fn shifted_palette() -> Rc<Theme> {
    let palette = corpus().palette;
    let foreground = palette
        .foreground
        .into_iter()
        .map(|(role, index)| (ThemeColor::Named(role), ColorValue::Index(index + 100)));
    let background = palette
        .background
        .into_iter()
        .map(|(role, index)| (ThemeBg::Named(role), ColorValue::Index(index)));
    Rc::new(
        Theme::new(
            foreground,
            background,
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}

/// The top border painted the way a recorded row paints `BORDER`.
fn bordered(row: &str) -> String {
    row.replace("BORDER", "─").repeat(20)
}

#[test]
fn transcript_initial_replay_updates_footer_history_and_compaction_notice() {
    let test = "transcript_initial_replay_updates_footer_history_and_compaction_notice";
    isolated(test, async |directory| {
        for (input, expected) in cases::<InitialInput, Transcript>(test) {
            assert_eq!(input.op, "initial");
            let world = recorded(directory, Setup::default());
            let content = json!([call("call", "unknown_tool")]);
            let ids = append(
                &world,
                vec![
                    user_record("HISTORY"),
                    assistant(&content, "toolUse", None),
                    result("call"),
                ],
            );
            for _ in 0..input.count {
                let session = world.host.session();
                let mut manager = session.session_manager().borrow_mut();
                manager
                    .append_compaction("SUMMARY", &ids[0], 10.0, CompactionOptions::default())
                    .unwrap();
            }
            let stale = crate::ToolExecutionComponent::new(tool_params(&world, "stale")).unwrap();
            world
                .mode
                .pending_tools
                .borrow_mut()
                .insert("stale".into(), Rc::new(stale));
            frames(&world);
            assert_ne!(border(&world), painted_border("thinkingOff"));
            world.mode.render_initial_messages().unwrap();
            assert_eq!(border(&world), painted_border("thinkingOff"));
            let rows = world.mode.chat_container.render(100);
            // Each compaction entry leaves its summary ahead of the kept messages.
            assert!(rows.ends_with(&expected.rows), "{rows:?}");
            assert_eq!(rows.len() > expected.rows.len(), input.count > 0);
            assert_eq!(pending(&world), expected.pending);
            assert_eq!(frames(&world), 1);
            assert_eq!(history(&world), expected.history);
        }
    });
}

/// Owned inputs for a stand-alone tool row of `id`.
pub(super) fn tool_params(world: &World, id: &str) -> crate::ToolExecutionParams {
    crate::ToolExecutionParams {
        tool_name: "unknown_tool".into(),
        tool_call_id: id.into(),
        args: None,
        options: crate::ToolExecutionOptions::default(),
        tool_definition: None,
        built_in_tool_definition: None,
        environment: Rc::clone(&world.mode.tool_environment),
        cwd: String::new(),
        request_render: Rc::new(|| {}),
    }
}

/// The initial thinking preference before the toggle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThinkingInput {
    /// The operation name.
    op: String,
    /// Whether thinking starts hidden.
    initial: bool,
    /// Whether a streaming tail is present; only cases without one are recorded here.
    streaming: bool,
}

#[test]
fn transcript_thinking_toggle_rebuilds_pending_rows_and_keeps_streaming_tail() {
    let test = "transcript_thinking_toggle_rebuilds_pending_rows_and_keeps_streaming_tail";
    isolated(test, async |directory| {
        for (input, expected) in cases::<ThinkingInput, Transcript>(test) {
            assert_eq!((input.op.as_str(), input.streaming), ("thinking", false));
            let initial = input.initial;
            let setup = Setup {
                configure: Box::new(move |settings| settings.set_hide_thinking_block(initial)),
                ..Setup::default()
            };
            let world = recorded(directory, setup);
            let content = json!([{"type": "thinking", "thinking": "REASON"},
                {"type": "text", "text": "ANSWER"}, call("call", "unknown_tool")]);
            append(
                &world,
                vec![user_record("HISTORY"), assistant(&content, "toolUse", None)],
            );
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            let before = history(&world);
            world.terminal.send_input("\x14");
            assert!(world.runtime.drain_futures().is_empty());
            let settings = world.host.session().settings_manager().clone();
            assert_eq!(
                Some(settings.borrow().get_hide_thinking_block()),
                expected.hidden
            );
            assert_eq!(world.mode.hide_thinking_block.get(), !initial);
            assert_eq!(world.mode.chat_container.render(100), expected.rows);
            assert_eq!(pending(&world), expected.pending);
            assert_eq!(frames(&world), 1);
            assert_eq!(history(&world), before);
        }
    });
}

/// Shell mode and the session's thinking level.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderInput {
    /// The operation name.
    op: String,
    /// Whether the editor holds a shell command.
    bash: bool,
    /// The thinking level.
    level: String,
}

/// The painted border word and the frames requested.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bordered {
    /// `BORDER` painted by the selected colour.
    rows: Vec<String>,
    /// Frames requested.
    renders: usize,
}

#[test]
fn transcript_footer_refresh_selects_the_current_mode_border() {
    let test = "transcript_footer_refresh_selects_the_current_mode_border";
    isolated(test, async |directory| {
        for (input, expected) in cases::<BorderInput, Bordered>(test) {
            assert_eq!(input.op, "border");
            let world = recorded(directory, Setup::default());
            // Starting the mode installs the callback that follows the published theme.
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            let level = || serde_json::from_value(Value::String(input.level.clone())).unwrap();
            let session = world.host.session();
            session.state().write().unwrap().thinking_level = level();
            world.mode.is_bash_mode.set(input.bash);
            frames(&world);
            world.mode.update_editor_border_color().unwrap();
            let [row] = <[String; 1]>::try_from(expected.rows).unwrap();
            assert_eq!(border(&world), bordered(&row));
            assert_eq!(frames(&world), expected.renders);
            world.themes.set_theme_instance(palette()).unwrap();
            assert_eq!(border(&world), bordered(&row));
            // Another palette recolours the same selection through the callback alone.
            let recorded_border = border(&world);
            world.themes.set_theme_instance(shifted_palette()).unwrap();
            let shifted = border(&world);
            assert_ne!(shifted, recorded_border, "the border keeps the old palette");
            let reference = recorded(directory, Setup::default());
            let session = reference.host.session();
            session.state().write().unwrap().thinking_level = level();
            reference.mode.is_bash_mode.set(input.bash);
            reference
                .themes
                .set_theme_instance(shifted_palette())
                .unwrap();
            reference.mode.update_editor_border_color().unwrap();
            assert_eq!(
                shifted,
                border(&reference),
                "the border follows the new palette"
            );
        }
    });
}

/// Which replay effects are selected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptionsInput {
    /// The operation name.
    op: String,
    /// Whether the footer and border are refreshed.
    update_footer: bool,
    /// Whether user messages join the history.
    populate_history: bool,
}

#[test]
fn transcript_replay_options_do_not_repopulate_history_on_rebuild() {
    let test = "transcript_replay_options_do_not_repopulate_history_on_rebuild";
    isolated(test, async |directory| {
        for (input, expected) in cases::<OptionsInput, Transcript>(test) {
            assert_eq!(input.op, "options");
            let world = recorded(directory, Setup::default());
            let stale = crate::ToolExecutionComponent::new(tool_params(&world, "stale")).unwrap();
            world
                .mode
                .pending_tools
                .borrow_mut()
                .insert("stale".into(), Rc::new(stale));
            let calls = assistant(&json!([call("new", "unknown_tool")]), "toolUse", None);
            append(&world, vec![user_record("HISTORY"), calls]);
            frames(&world);
            let options = super::super::replay::RenderSessionContextOptions {
                update_footer: input.update_footer,
                populate_history: input.populate_history,
            };
            let session = world.host.session();
            let context = session
                .session_manager()
                .borrow()
                .build_session_context()
                .unwrap();
            world.mode.render_session_context(context, options).unwrap();
            assert_eq!(
                border(&world) == painted_border("thinkingOff"),
                input.update_footer
            );
            observe(&world, &expected, 0);
            world.mode.editor.add_to_history("LATER");
            world.mode.rebuild_chat_from_messages().unwrap();
            assert_eq!(world.mode.chat_container.render(100), expected.rows);
            assert_eq!(pending(&world), expected.pending);
            let mut later = expected.history.clone();
            later.push("LATER".into());
            assert_eq!(history(&world), later);
        }
    });
}

/// A registered tool `name` without renderers, returning nothing.
pub(super) type Definition = maestro_app::local::ToolDefinition<ExtensionContext, Value, Value, ()>;

/// A registered tool `name` without renderers whose execution returns nothing.
pub(super) fn tool(name: &str) -> Definition {
    Definition {
        definition: serde_json::from_value(json!({"name": name, "description": "",
            "parameters": {"type": "object", "properties": {}}}))
        .unwrap(),
        label: name.into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: None,
                    terminate: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// An extension registering `tools`.
pub(super) fn registering(tools: Vec<Definition>) -> ExtensionFactory<Local> {
    let tools = RefCell::new(tools);
    Rc::new(move |api| {
        for tool in tools.take() {
            api.register_tool(tool).unwrap();
        }
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A result renderer reading the raw stored result through `render`.
fn raw(
    render: impl Fn(&ToolRenderResultLike<'_, Value>) -> ComponentHandle + 'static,
) -> ToolRenderResult<Value, ()> {
    ToolRenderResult::<Value, ()>::Raw(Rc::new(move |result, _, _, _| Ok(render(result))))
}

/// A recorded text row as a renderer's component.
pub(super) fn text(value: &str) -> ComponentHandle {
    Rc::new(Text::new(value.into(), 0, 0, None))
}

/// Image visibility with a Markdown indent, or with an image width.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SettingsInput {
    /// `settings` or `image_settings`.
    op: String,
    /// Whether images are shown.
    show_images: bool,
    /// The code-block indent.
    #[serde(default)]
    indent: Option<String>,
    /// The image width in cells.
    #[serde(default)]
    width: Option<f64>,
}

/// A one-pixel image as a recorded tool result.
fn image_result() -> Value {
    let data = [
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/",
        "AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==",
    ]
    .concat();
    let mut record = result("call");
    record["content"] = json!([{"type": "image", "mimeType": "image/png", "data": data}]);
    record
}

/// A mode whose settings select the recorded indent, image width and image visibility, with
/// `custom` registered.
fn settings_setup(input: &SettingsInput, custom: Definition) -> Setup {
    let (show, indent, width) = (input.show_images, input.indent.clone(), input.width);
    Setup {
        configure: Box::new(move |settings| {
            settings.set_show_images(show);
            if let Some(indent) = indent {
                settings.set_global_key("markdown", "codeBlockIndent", indent);
            }
            if let Some(width) = width {
                settings.set_image_width_cells(width);
            }
        }),
        extension: Some(registering(vec![custom])),
        ..Setup::default()
    }
}

#[test]
fn transcript_tool_settings_reach_renderers_independently() {
    let test = "transcript_tool_settings_reach_renderers_independently";
    isolated(test, async |directory| {
        for (input, expected) in cases::<SettingsInput, Transcript>(test) {
            let events = Rc::new(RefCell::new(Vec::new()));
            let mut custom = tool("custom");
            let seen = Rc::clone(&events);
            custom.render_call = Some(Rc::new(move |_, _, context| {
                seen.borrow_mut()
                    .push(json!({"showImages": context.show_images,
                    "cwd": context.cwd, "id": context.tool_call_id}));
                Ok(text("REGISTERED"))
            }));
            let world = recorded(directory, settings_setup(&input, custom));
            frames(&world);
            if input.op == "image_settings" {
                world
                    .mode
                    .tool_environment
                    .terminal_image
                    .set_capabilities(TerminalCapabilities {
                        images: Some(ImageProtocol::Iterm2),
                        true_color: true,
                        hyperlinks: false,
                    });
                let content = json!([call("call", "unknown_tool")]);
                replay(
                    &world,
                    vec![assistant(&content, "toolUse", None), image_result()],
                );
            } else {
                let content = json!([call("call", "custom")]);
                let code = user_record("```unknown\nCODE\n```");
                replay(&world, vec![assistant(&content, "toolUse", None), code]);
            }
            let cwd = directory.to_str().unwrap();
            let events: Vec<Value> = events
                .borrow()
                .iter()
                .map(|event| {
                    let mut event = event.clone();
                    assert_eq!(event["cwd"], cwd);
                    event["cwd"] = json!("/work");
                    event
                })
                .collect();
            assert_eq!(events, expected.events.clone().unwrap_or_default());
            observe(&world, &expected, 0);
        }
    });
}

/// A tool name and which of its renderers the registration overrides.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupInput {
    /// The operation name.
    op: String,
    /// A built-in tool name or `unknown_tool`.
    name: String,
    /// `none`, `call` or `result`.
    r#override: String,
}

#[test]
fn transcript_registered_tool_and_builtin_fallbacks_keep_one_lookup() {
    let test = "transcript_registered_tool_and_builtin_fallbacks_keep_one_lookup";
    isolated(test, async |directory| {
        for (input, expected) in cases::<LookupInput, Transcript>(test) {
            assert_eq!(input.op, "lookup");
            let mut registered = tool(&input.name);
            match input.r#override.as_str() {
                "call" => registered.render_call = Some(Rc::new(|_, _, _| Ok(text("CUSTOM_CALL")))),
                "result" => {
                    registered.render_result = Some(raw(|_| text("CUSTOM_RESULT")));
                }
                _ => {}
            }
            let extension = (input.r#override != "none").then(|| registering(vec![registered]));
            let world = recorded(
                directory,
                Setup {
                    extension,
                    ..Setup::default()
                },
            );
            frames(&world);
            let mut native = call("native", &input.name);
            native["arguments"] = json!({"path": "controlled.txt", "command": "echo value",
                "pattern": "needle"});
            replay(
                &world,
                vec![
                    assistant(&json!([native]), "toolUse", None),
                    result("native"),
                ],
            );
            observe(&world, &expected, 0);
        }
    });
}

/// The kind of a result's details.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KindInput {
    /// The operation name.
    op: String,
    /// The JSON kind, or `missing`.
    kind: String,
}

/// The recorded JSON value of `kind`.
pub(super) fn kind_value(kind: &str) -> Value {
    match kind {
        "null" => Value::Null,
        "string" => json!("detail"),
        "number" => json!(-0.0),
        "boolean" => json!(false),
        "object" => json!({"z": 1, "a": 2}),
        "array" => json!([1, 2]),
        other => panic!("unrecorded kind {other}"),
    }
}

#[test]
fn transcript_message_field_domains_use_the_existing_boundary() {
    let test = "transcript_message_field_domains_use_the_existing_boundary";
    isolated(test, async |directory| {
        for (input, expected) in cases::<KindInput, Transcript>(test) {
            assert_eq!(input.op, "details");
            let events = Rc::new(RefCell::new(Vec::new()));
            let mut registered = tool("unknown_tool");
            let seen = Rc::clone(&events);
            registered.render_result = Some(raw(move |result| {
                seen.borrow_mut().push(json!({"hasDetails": true,
                    "detailsUndefined": result.details.is_none(),
                    "details": result.details.cloned().unwrap_or(Value::Null)}));
                text("RESULT")
            }));
            let setup = Setup {
                extension: Some(registering(vec![registered])),
                ..Setup::default()
            };
            let world = recorded(directory, setup);
            frames(&world);
            let mut stored = result("call");
            if input.kind != "missing" {
                stored["details"] = kind_value(&input.kind);
            }
            let content = json!([call("call", "unknown_tool")]);
            replay(&world, vec![assistant(&content, "toolUse", None), stored]);
            let mut last = events.borrow().last().cloned().unwrap();
            if input.kind == "number" {
                // The recording's JSON text writes negative zero as 0; the value keeps its sign.
                assert!(last["details"].as_f64().unwrap().is_sign_negative());
                last["details"] = json!(0);
            }
            assert_eq!(Some(&last), expected.events.as_ref().and_then(|e| e.last()));
            observe(&world, &expected, 0);
        }
    });
}

#[test]
fn transcript_tool_argument_domains_reach_the_existing_renderer() {
    let test = "transcript_tool_argument_domains_reach_the_existing_renderer";
    isolated(test, async |directory| {
        for (input, expected) in cases::<KindInput, Transcript>(test) {
            assert_eq!(input.op, "argument_domain");
            let world = recorded(directory, Setup::default());
            let mut stored = call("call", "unknown_tool");
            stored["arguments"] = match input.kind.as_str() {
                "string" => json!("args"),
                kind => kind_value(kind),
            };
            append(
                &world,
                vec![assistant(&json!([stored]), "toolUse", None), result("call")],
            );
            frames(&world);
            world.mode.render_initial_messages().unwrap();
            observe(&world, &expected, 0);
        }
    });
}

/// One assistant record's thinking preference and failure kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceInput {
    /// The operation name.
    op: String,
    /// Whether thinking starts hidden.
    hide: bool,
    /// `mixed` content or a failure kind.
    kind: String,
}

/// The rows after construction, a visibility change, a label change and an update.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Phases {
    /// Rows after construction.
    before: Vec<String>,
    /// Rows after the visibility change.
    toggled: Vec<String>,
    /// Rows after the label change.
    labelled: Vec<String>,
    /// Rows after a typed live update.
    updated: Vec<String>,
}

/// The stored assistant record of a source case.
fn source_record(kind: &str) -> Value {
    let content = if kind == "mixed" {
        json!([{"type": "thinking", "thinking": "  REASON  "},
            {"type": "text", "text": "  ANSWER  "}])
    } else {
        json!([{"type": "text", "text": "PARTIAL"}])
    };
    let stop = match kind {
        "mixed" => "stop",
        kind if kind.starts_with("error") => "error",
        _ => "aborted",
    };
    let error = match kind {
        "error" => Some("DETAIL"),
        "error_empty" => Some(""),
        "abort_custom" => Some("ABORT_DETAIL"),
        "abort_sentinel" => Some("Request was aborted"),
        _ => None,
    };
    assistant(&content, stop, error)
}

#[test]
fn transcript_recorded_assistant_keeps_the_existing_paint_owner() {
    let test = "transcript_recorded_assistant_keeps_the_existing_paint_owner";
    isolated(test, async |directory| {
        let world = recorded(directory, Setup::default());
        for (input, expected) in cases::<SourceInput, Phases>(test) {
            assert_eq!(input.op, "assistant_source");
            let record = Rc::new(source_record(&input.kind));
            let released = Rc::downgrade(&record);
            let AgentMessage::Assistant(live) = message((*record).clone()) else {
                panic!("an assistant record");
            };
            let options = || crate::AssistantMessageOptions {
                hide_thinking_block: input.hide,
                ..crate::AssistantMessageOptions::default()
            };
            let theme = || world.themes.theme();
            let image = || world.mode.tool_environment.terminal_image.clone();
            let markdown = || world.mode.get_markdown_theme_with_settings();
            let rows = [
                crate::AssistantMessageComponent::from_record(
                    record,
                    options(),
                    theme(),
                    markdown(),
                    image(),
                ),
                crate::AssistantMessageComponent::new(
                    Some(live),
                    options(),
                    theme(),
                    markdown(),
                    image(),
                ),
            ];
            let AgentMessage::Assistant(next) = message(assistant(
                &json!([{"type": "text", "text": "NEXT"}]),
                "stop",
                None,
            )) else {
                panic!("an assistant record");
            };
            for row in &rows {
                assert_eq!(row.render(100), expected.before);
                row.set_hide_thinking_block(!input.hide);
                assert_eq!(row.render(100), expected.toggled);
                row.set_hidden_thinking_label("HIDDEN_LABEL".into());
                assert_eq!(row.render(100), expected.labelled);
                row.update_content(next.clone());
                assert_eq!(row.render(100), expected.updated);
            }
            assert!(
                released.upgrade().is_none(),
                "the recorded source is released"
            );
        }
    });
}

#[test]
fn transcript_length_stop_shows_the_incomplete_response_error() {
    let test = "transcript_length_stop_shows_the_incomplete_response_error";
    isolated(test, async |directory| {
        let world = recorded(directory, Setup::default());
        let truncated_call = json!([{"type": "text", "text": "PARTIAL"}, call("1", "read")]);
        for content in [json!([{"type": "text", "text": "PARTIAL"}]), truncated_call] {
            let record = Rc::new(assistant(&content, "length", None));
            let AgentMessage::Assistant(live) = message((*record).clone()) else {
                panic!("an assistant record");
            };
            let rows = [
                crate::AssistantMessageComponent::from_record(
                    record,
                    crate::AssistantMessageOptions::default(),
                    world.themes.theme(),
                    world.mode.get_markdown_theme_with_settings(),
                    world.mode.tool_environment.terminal_image.clone(),
                ),
                crate::AssistantMessageComponent::new(
                    Some(live),
                    crate::AssistantMessageOptions::default(),
                    world.themes.theme(),
                    world.mode.get_markdown_theme_with_settings(),
                    world.mode.tool_environment.terminal_image.clone(),
                ),
            ];
            for row in &rows {
                let rendered = row.render(200).join("\n");
                assert!(
                    rendered.contains(
                        "Error: Model stopped because it reached the maximum output token limit. The response may be incomplete."
                    ),
                    "{content}: {rendered}"
                );
            }
        }
    });
}

/// Renderer calls of `count` replayed call and result pairs, before and after expansion,
/// with the transcript's row count. The calls are the only work this counts: a collection
/// walk that is quadratic in the rows would not show in them.
fn replay_work(directory: &Path, count: usize) -> (usize, usize, usize) {
    let calls = Rc::new(Cell::new(0));
    let mut counted = tool("counted");
    let seen = Rc::clone(&calls);
    counted.render_call = Some(Rc::new(move |_, _, _| {
        seen.set(seen.get() + 1);
        Ok(text("CALL"))
    }));
    let seen = Rc::clone(&calls);
    counted.render_result = Some(raw(move |_| {
        seen.set(seen.get() + 1);
        text("RESULT")
    }));
    let setup = Setup {
        extension: Some(registering(vec![counted])),
        ..Setup::default()
    };
    let world = recorded(directory, setup);
    let messages = (0..count)
        .flat_map(|index| {
            let id = index.to_string();
            [
                assistant(&json!([call(&id, "counted")]), "toolUse", None),
                result(&id),
            ]
        })
        .collect();
    replay(&world, messages);
    let replayed = calls.get();
    world.mode.set_tools_expanded(true).unwrap();
    let rows = world.mode.chat_container.children().borrow().len();
    (replayed, calls.get() - replayed, rows)
}

#[test]
fn transcript_replay_and_expansion_bound_renderer_calls() {
    isolated(
        "transcript_replay_and_expansion_bound_renderer_calls",
        async |directory| {
            let work: Vec<_> = [16, 32, 64]
                .into_iter()
                .map(|count| replay_work(directory, count))
                .collect();
            for pair in work.windows(2) {
                let ((replay, expand, rows), (next_replay, next_expand, next_rows)) =
                    (pair[0], pair[1]);
                assert_eq!(next_rows, 2 * rows);
                assert!(next_replay <= 2 * replay, "{work:?}");
                assert!(next_expand <= 2 * expand, "{work:?}");
            }
        },
    );
}

#[test]
fn transcript_batched_results_keep_pending_call_order() {
    isolated(
        "transcript_batched_results_keep_pending_call_order",
        async |directory| {
            for count in [16, 32, 64] {
                let world = recorded(directory, Setup::default());
                let id = |index: usize| format!("c{index}");
                let calls: Vec<_> = (0..count)
                    .map(|index| call(&id(index), "unknown_tool"))
                    .collect();
                let last = id(count - 1);
                // The batch's results arrive in call order after every call; `c1` is called
                // again while pending, `c0` again after its result, and two calls stay open.
                let mut messages = vec![
                    assistant(&Value::Array(calls), "toolUse", None),
                    assistant(&json!([call("c1", "unknown_tool")]), "toolUse", None),
                ];
                messages.extend(
                    (0..count)
                        .map(id)
                        .filter(|key| key != "c1" && *key != last)
                        .map(|key| result(&key)),
                );
                messages.push(assistant(
                    &json!([call("c0", "unknown_tool")]),
                    "toolUse",
                    None,
                ));
                replay(&world, messages);
                let rows = world.mode.chat_container.render(100);
                let completed = rows
                    .iter()
                    .filter(|row| row.contains("HISTORICAL_RESULT"))
                    .count();
                assert_eq!(completed, count - 2, "{rows:#?}");
                assert_eq!(pending(&world), ["c1".to_owned(), last, "c0".to_owned()]);
            }
        },
    );
}
