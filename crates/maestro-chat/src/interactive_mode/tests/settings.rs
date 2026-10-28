//! Live settings through the panel the mode shows in place of its editor.
use super::models::model_input;
use super::transcript;
use super::typed::{poll, recalled, submit};
use super::{Setup, World, isolated, start};
use crate::SettingsSelectorComponent;
use maestro_app::agent_session::PromptOptions;
use maestro_app::presentation_data::footer_data_provider::FooterOperations;
use maestro_app::{
    AssistantMessageEventStream, MessageDeliveryMode, SettingsManager, Transport, TransportSetting,
    strip_ansi,
};
use maestro_theme::{ThemeColor, ThemeDirectories};
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, SettingsList, TerminalCapabilities};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// The shipped themes every panel inventory reads.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../maestro-theme/assets/theme");

/// The theme directories over the shipped themes and the custom directory of `directory`.
fn registry(directory: &Path) -> ThemeDirectories {
    let custom = directory.join("themes");
    if !custom.exists() {
        std::fs::create_dir(&custom).unwrap();
    }
    ThemeDirectories {
        themes_dir: SHIPPED.to_owned(),
        custom_themes_dir: custom.to_str().unwrap().to_owned(),
    }
}

/// A started mode whose panel reads a real theme inventory.
async fn settings_world(
    directory: &Path,
    configure: impl FnOnce(&mut SettingsManager) + 'static,
) -> World {
    let world = Setup {
        directories: Some(registry(directory)),
        configure: Box::new(configure),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    world
}

/// A default started mode whose panel reads a real theme inventory.
async fn started_settings(directory: &Path) -> World {
    settings_world(directory, |_| {}).await
}

/// The settings panel the editor container shows, if it shows one.
fn panel(world: &World) -> Option<Rc<SettingsSelectorComponent>> {
    let children = world.mode.editor_container.children().borrow().clone();
    let [child] = children.as_slice() else {
        return None;
    };
    (Rc::clone(child) as Rc<dyn std::any::Any>)
        .downcast::<SettingsSelectorComponent>()
        .ok()
}

/// Opens settings through the mode's own operation and returns the shown list.
fn open(world: &World) -> Rc<SettingsList> {
    world.mode.show_settings_selector().unwrap();
    open_list(world)
}

/// The list of the panel already shown.
fn open_list(world: &World) -> Rc<SettingsList> {
    Rc::clone(
        panel(world)
            .expect("the panel is shown")
            .get_settings_list(),
    )
}

/// The saved form of `transport`.
fn saved(transport: &Transport) -> TransportSetting {
    match transport {
        Transport::Auto => TransportSetting::Auto,
        Transport::Sse => TransportSetting::Sse,
        Transport::Websocket => TransportSetting::Websocket,
        Transport::WebsocketCached => TransportSetting::WebsocketCached,
    }
}

/// Delivers `data` to the panel's installed list.
fn press(list: &SettingsList, data: &str) {
    list.handle_input(data);
}

/// Clears the list's search and types `query`, leaving only its matches.
fn search(list: &SettingsList, query: &str) {
    for _ in 0..16 {
        press(list, "\u{7f}");
    }
    for character in query.chars() {
        press(list, &character.to_string());
    }
}

/// Activates the `match_index`-th row the query leaves, in panel order.
fn choose(list: &SettingsList, query: &str, match_index: usize) {
    search(list, query);
    for _ in 0..match_index {
        press(list, "\x1b[B");
    }
    press(list, "\r");
}

/// The panel's rendered rows.
fn rows(world: &World) -> Vec<String> {
    panel(world).expect("the panel is shown").render(100)
}

/// Whether the panel's rendered rows hold a row starting with `label` and carrying `value`.
fn shows_value(world: &World, label: &str, value: &str) -> bool {
    rows(world).iter().any(|row| {
        let text = strip_ansi(row).unwrap();
        let text = text.trim_start_matches(|character: char| !character.is_alphanumeric());
        text.starts_with(label) && text.split_whitespace().last() == Some(value)
    })
}

#[test]
fn settings_snapshot_reads_live_session_and_preferences() {
    isolated(
        "settings_snapshot_reads_live_session_and_preferences",
        async |directory| {
            let world = settings_world(directory, |settings| {
                settings.set_show_images(false);
                settings.set_image_width_cells(80.0);
                settings.set_theme("light".to_owned());
            })
            .await;
            let session = world.host.session();
            session.set_auto_compaction_enabled(true);
            session
                .set_steering_mode(maestro_app::QueueMode::All)
                .unwrap();
            session
                .set_follow_up_mode(maestro_app::QueueMode::OneAtATime)
                .unwrap();
            let levels = session.get_available_thinking_levels();
            open(&world);
            let config = world.mode.settings_config().unwrap();
            assert!(config.session.auto_compact);
            assert!(!config.session.quiet_startup);
            assert_eq!(config.steering_mode, MessageDeliveryMode::All);
            assert_eq!(config.follow_up_mode, MessageDeliveryMode::OneAtATime);
            assert_eq!(config.thinking_level, session.thinking_level());
            assert_eq!(
                config.available_thinking_levels, levels,
                "the panel offers the model's own levels"
            );
            assert!(!config.show_images);
            assert!(config.image_width_cells.total_cmp(&80.0).is_eq());
            assert_eq!(config.current_theme, "light");
            assert_eq!(
                config.available_themes,
                ["dark".to_owned(), "light".to_owned()],
                "the panel lists the real inventory"
            );
            assert_eq!(
                config.available_thinking_levels,
                session.get_available_thinking_levels()
            );
            let list = open_list(&world);
            search(&list, "Transport");
            assert!(shows_value(&world, "Transport", "auto"));
            search(&list, "Theme");
            assert!(shows_value(&world, "Theme", "light"));
            search(&list, "Auto-compact");
            assert!(shows_value(&world, "Auto-compact", "true"));
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_snapshot_covers_every_source_field() {
    isolated(
        "settings_snapshot_covers_every_source_field",
        async |directory| {
            // A terminal that reports image support admits the inline image rows; a plain
            // one leaves them out.
            let world = started_settings(directory).await;
            world
                .mode
                .tool_environment
                .terminal_image
                .set_capabilities(TerminalCapabilities::default());
            open(&world);
            let rows = rows(&world);
            assert!(rows.iter().any(|row| row.contains("resize")));
            assert!(!rows.iter().any(|row| row.contains("Show images")));
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_cancel_requests_a_frame_after_restoring_editor() {
    isolated(
        "settings_cancel_requests_a_frame_after_restoring_editor",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            assert!(panel(&world).is_some());
            press(&list, "\x1b");
            assert!(panel(&world).is_none(), "the panel is removed");
            let children = world.mode.editor_container.children().borrow().clone();
            assert_eq!(children.len(), 1);
            assert!(std::ptr::addr_eq(
                Rc::as_ptr(&children[0]),
                Rc::as_ptr(&world.mode.editor).cast::<()>()
            ));
            world.mode.editor.set_text("");
            world.terminal.send_input("z");
            assert_eq!(world.mode.editor.get_text(), "z", "the editor is focused");
            assert!(world.runtime.pending() > 0, "a frame is requested");
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_command_uses_trimmed_exact_token_and_clears_editor() {
    isolated(
        "settings_command_uses_trimmed_exact_token_and_clears_editor",
        async |directory| {
            let world = started_settings(directory).await;
            for text in ["/settings", " /settings\t", "\u{feff}/settings "] {
                world.mode.editor.set_text(text);
                submit(&world.mode, text);
                assert!(panel(&world).is_some(), "{text:?} opens the panel");
                press(&open_list(&world), "\x1b");
                assert!(panel(&world).is_none(), "the panel closes between cases");
            }
            world.mode.editor.set_text("/settings");
            world.mode.editor.handle_input("\r");
            assert!(panel(&world).is_some(), "the key route opens the panel");
            assert_eq!(world.mode.editor.get_text(), "", "text is cleared");
            press(&open_list(&world), "\x1b");
            for text in [
                "/settings all",
                "/settingsall",
                "/Settings",
                "\u{85}/settings",
                "\u{200b}/settings",
            ] {
                world.mode.editor.set_text(text);
                submit(&world.mode, text);
                assert!(panel(&world).is_none(), "{text:?} does not open the panel");
                assert_eq!(world.mode.editor.get_text(), text, "text stays");
            }
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_changes_preserve_each_callback_effect() {
    isolated(
        "settings_changes_preserve_each_callback_effect",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let session = world.host.session();
            assert!(footer_shows(&world, "(auto)"));
            choose(&list, "compact", 0);
            assert!(!session.auto_compaction_enabled(), "the session saved it");
            assert!(!footer_shows(&world, "(auto)"), "the footer learned it");
            choose(&list, "resize", 0);
            assert!(!session.settings_manager().borrow().get_image_auto_resize());
            choose(&list, "block", 0);
            assert!(session.settings_manager().borrow().get_block_images());
            choose(&list, "skill", 0);
            assert!(
                !session
                    .settings_manager()
                    .borrow()
                    .get_enable_skill_commands()
            );
            choose(&list, "quiet", 0);
            assert!(session.settings_manager().borrow().get_quiet_startup());
            choose(&list, "changelog", 0);
            assert!(session.settings_manager().borrow().get_collapse_changelog());
            choose(&list, "telemetry", 0);
            assert!(
                !session
                    .settings_manager()
                    .borrow()
                    .get_enable_install_telemetry()
            );
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_remaining_changes_preserve_each_callback_effect() {
    isolated(
        "settings_remaining_changes_preserve_each_callback_effect",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let session = world.host.session();
            choose(&list, "Escape", 0);
            assert_eq!(
                session
                    .settings_manager()
                    .borrow()
                    .get_double_escape_action(),
                maestro_app::DoubleEscapeAction::Fork
            );
            choose(&list, "filter", 0);
            assert_eq!(
                session.settings_manager().borrow().get_tree_filter_mode(),
                maestro_app::TreeFilterMode::NoTools
            );
            choose(&list, "shrink", 0);
            assert!(session.settings_manager().borrow().get_clear_on_shrink());
            choose(&list, "progress", 0);
            assert!(
                session
                    .settings_manager()
                    .borrow()
                    .get_show_terminal_progress()
            );
            let cursor = session
                .settings_manager()
                .borrow()
                .get_show_hardware_cursor();
            choose(&list, "hardware", 0);
            let saved = session
                .settings_manager()
                .borrow()
                .get_show_hardware_cursor();
            assert_ne!(saved, cursor, "the cursor choice is saved");
            assert!(
                !session.settings_manager().borrow().get_block_images(),
                "unrelated settings are unchanged"
            );
            search(&list, "Terminal");
            assert!(shows_value(&world, "Terminal progress", "true"));
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_session_modes_use_distinct_live_policies() {
    isolated(
        "settings_session_modes_use_distinct_live_policies",
        async |directory| {
            let world = settings_world(directory, |_| {}).await;
            let session = world.host.session();
            session
                .set_steering_mode(maestro_app::QueueMode::All)
                .unwrap();
            session
                .set_follow_up_mode(maestro_app::QueueMode::OneAtATime)
                .unwrap();
            let list = open(&world);
            choose(&list, "Steering", 0);
            assert_eq!(session.steering_mode(), maestro_app::QueueMode::OneAtATime);
            assert_eq!(
                session.follow_up_mode(),
                maestro_app::QueueMode::OneAtATime,
                "the sibling policy is unchanged"
            );
            choose(&list, "follow", 0);
            assert_eq!(session.follow_up_mode(), maestro_app::QueueMode::All);
            assert_eq!(
                session.steering_mode(),
                maestro_app::QueueMode::OneAtATime,
                "the sibling policy is unchanged in the other direction"
            );
            drop(world.runtime.settle());
        },
    );
}

/// Each of the four choices must reach the live agent the session executes prompts on, so
/// the next request is built with it: the session prompts `self.agent`
/// (`agent_session/prompting.rs:73`) and the request options read that same agent's
/// `transport` (`maestro-agent/src/agent/execution.rs:234`).
#[test]
fn settings_transport_change_reaches_the_live_agent() {
    isolated(
        "settings_transport_change_reaches_the_live_agent",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let session = world.host.session();
            for transport in [
                Transport::Sse,
                Transport::Websocket,
                Transport::WebsocketCached,
                Transport::Auto,
            ] {
                choose(&list, "Transport", 0);
                assert_eq!(
                    session.settings_manager().borrow().get_transport(),
                    saved(&transport),
                    "the selection is saved"
                );
                let live = session
                    .agent()
                    .transport
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                assert_eq!(live, transport);
            }
            drop(world.runtime.settle());
        },
    );
}

/// Each of the four choices must reach the request the next prompt sends: the controlled
/// provider records the transport its options carry, so the agent field alone proves nothing.
#[test]
fn settings_transport_change_reaches_next_model_request() {
    isolated(
        "settings_transport_change_reaches_next_model_request",
        async |directory| {
            let world = started_settings(directory).await;
            world
                .host
                .services()
                .auth_storage
                .set_runtime_api_key("unknown", "key");
            let list = open(&world);
            let session = world.host.session();
            let requested: Rc<RefCell<Vec<Option<Transport>>>> = Rc::default();
            let record = Rc::clone(&requested);
            *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, _, options, _| {
                record.borrow_mut().push(options.common.transport);
                let stream = AssistantMessageEventStream::new();
                stream.end(Some(super::answer()));
                Box::pin(std::future::ready(Ok(stream)))
            }));
            for transport in [
                Transport::Sse,
                Transport::Websocket,
                Transport::WebsocketCached,
                Transport::Auto,
            ] {
                choose(&list, "Transport", 0);
                session
                    .prompt("hello", PromptOptions::default())
                    .await
                    .unwrap();
                assert_eq!(requested.borrow().last(), Some(&Some(transport)));
            }
            assert_eq!(requested.borrow().len(), 4, "one request per prompt");
            drop(world.runtime.settle());
        },
    );
}

/// One call the counted tool's renderer served: the row's identity and the image visibility
/// it was rendered with.
type Served = (String, bool);

/// What the renderer of row `t1` runs once, set after the rows exist.
type Churn = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

/// A tool whose result renderer notes each call it serves, so the tests count every rebuild,
/// and whose row `t1` runs `churn` once when its rebuild comes.
fn counted_tool(seen: &Rc<RefCell<Vec<Served>>>, churn: &Churn) -> transcript::Definition {
    let mut counted = transcript::tool("counted");
    let (seen, churn) = (Rc::clone(seen), Rc::clone(churn));
    counted.render_result = Some(maestro_app::ToolRenderResult::<Value, ()>::Raw(Rc::new(
        move |_: &maestro_app::ToolRenderResultLike<'_, Value>,
              _: maestro_app::ToolRenderResultOptions,
              _: &maestro_theme::LiveTheme,
              context: &maestro_app::ToolRenderContext<'_, ()>| {
            seen.borrow_mut()
                .push((context.tool_call_id.clone(), context.show_images));
            if context.tool_call_id == "t1"
                && let Some(churn) = churn.borrow_mut().take()
            {
                churn();
            }
            Ok(transcript::text("RESULT"))
        },
    )));
    counted
}

/// A started mode over the real theme inventory whose session registers `counted_tool`.
async fn counted_world(directory: &Path, seen: &Rc<RefCell<Vec<Served>>>, churn: &Churn) -> World {
    let world = Setup {
        directories: Some(registry(directory)),
        extension: Some(transcript::registering(vec![counted_tool(seen, churn)])),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    // The panel offers the image rows only when the terminal shows images.
    world
        .mode
        .tool_environment
        .terminal_image
        .set_capabilities(TerminalCapabilities {
            images: Some(maestro_tui::ImageProtocol::Iterm2),
            ..TerminalCapabilities::default()
        });
    world
}

/// Replays `count` stored calls of the counted tool, `t0` first, each with its result.
fn replay_counted(world: &World, count: usize) {
    let messages = (0..count)
        .flat_map(|index| {
            let id = format!("t{index}");
            [
                transcript::assistant(&json!([transcript::call(&id, "counted")]), "toolUse", None),
                transcript::result(&id),
            ]
        })
        .collect();
    transcript::replay(world, messages);
}

/// The tool rows of the transcript, in order.
fn tool_rows_of(world: &World) -> Vec<Rc<crate::ToolExecutionComponent>> {
    world
        .mode
        .chat_container
        .walk()
        .filter_map(|child| {
            (child as Rc<dyn std::any::Any>)
                .downcast::<crate::ToolExecutionComponent>()
                .ok()
        })
        .collect()
}

/// The renderer calls one image-visibility choice makes over `count` retained rows, and the
/// calls of a deliberately rescanning update that re-walks every row for each row.
async fn image_choice_work(directory: &Path, count: usize) -> (usize, usize) {
    let seen = Rc::default();
    let world = counted_world(directory, &seen, &Churn::default()).await;
    replay_counted(&world, count);
    let list = open(&world);
    let before = seen.borrow().len();
    choose(&list, "images", 0);
    let work = seen.borrow().len() - before;
    let rows = tool_rows_of(&world);
    assert_eq!(rows.len(), count, "every replayed call is a retained row");
    let before = seen.borrow().len();
    for _ in &rows {
        for row in &rows {
            row.set_show_images(true).unwrap();
        }
    }
    let rescanned = seen.borrow().len() - before;
    drop(world.runtime.settle());
    (work, rescanned)
}

/// One image-visibility choice rebuilds each retained tool row once: the work doubles with
/// the rows, while an update that re-walks every row for each row grows faster than that.
#[test]
fn settings_tool_updates_have_linear_work() {
    isolated(
        "settings_tool_updates_have_linear_work",
        async |directory| {
            let mut work = Vec::new();
            for count in [16, 32, 64] {
                let (changed, rescanned) = image_choice_work(directory, count).await;
                assert!(
                    (count..=2 * count).contains(&changed),
                    "{count} rows: {changed} calls"
                );
                assert!(rescanned > 2 * changed, "the control rescans: {rescanned}");
                work.push(changed);
            }
            assert!(work[1] <= 2 * work[0] && work[2] <= 2 * work[1], "{work:?}");
        },
    );
}

/// How the callback running inside one row's rebuild changes the transcript's children.
#[derive(Clone, Copy, Debug)]
enum Edit {
    /// Appends a further tool row.
    Append,
    /// Removes the row being rebuilt and the transcript entry after it.
    RemoveWithNext,
    /// Clears the transcript.
    Clear,
    /// Gives the transcript another array holding one further tool row.
    Replace,
}

/// The ids the update visited in order, and the ids of the tool rows the transcript holds
/// afterwards, when row `t1` applies `edit` to the transcript during its rebuild and the
/// panel row matching `query` changes the setting.
async fn edited_update(directory: &Path, edit: Edit, query: &str) -> (Vec<String>, Vec<String>) {
    let seen = Rc::default();
    let churn = Churn::default();
    let world = counted_world(directory, &seen, &churn).await;
    replay_counted(&world, 4);
    let rows = tool_rows_of(&world);
    let mut named: Vec<(&str, Rc<crate::ToolExecutionComponent>)> =
        ["t0", "t1", "t2", "t3"].into_iter().zip(rows).collect();
    let binding = world
        .host
        .session()
        .get_tool_definition("counted")
        .unwrap()
        .renderer_binding()
        .unwrap();
    let mut params = transcript::tool_params(&world, "late");
    params.tool_name = "counted".into();
    params.tool_definition = Some(binding);
    let late = Rc::new(crate::ToolExecutionComponent::new(params).unwrap());
    late.update_result(image_result("late"), false).unwrap();
    named.push(("late", Rc::clone(&late)));
    let container = Rc::clone(&world.mode.chat_container);
    let current: maestro_tui::tui::ComponentHandle = Rc::clone(&named[1].1) as _;
    let late: maestro_tui::tui::ComponentHandle = late;
    *churn.borrow_mut() = Some(match edit {
        Edit::Append => Box::new(move || container.add_child(late)),
        Edit::RemoveWithNext => Box::new(move || {
            let array = container.children();
            let mut children = array.borrow_mut();
            let at = children
                .iter()
                .position(|child| Rc::ptr_eq(child, &current))
                .unwrap();
            children.drain(at..=at + 1);
        }),
        Edit::Clear => Box::new(move || container.clear()),
        Edit::Replace => Box::new(move || {
            container.set_children(Rc::new(RefCell::new(vec![late])));
        }),
    });
    let list = open(&world);
    let before = seen.borrow().len();
    choose(&list, query, 0);
    let visited = seen.borrow()[before..]
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    let held = tool_rows_of(&world);
    let kept = named
        .iter()
        .filter(|(_, row)| held.iter().any(|held| Rc::ptr_eq(held, row)))
        .map(|(id, _)| (*id).to_owned())
        .collect();
    drop(world.runtime.settle());
    (visited, kept)
}

/// The child walk a setting change makes is the live array it started on: a row appended or
/// removed during the walk is reached or skipped like the array's indexes say, and a cleared
/// or replaced transcript leaves the walk on its old rows. Image visibility and image width
/// take the same route.
#[test]
fn settings_tool_updates_keep_captured_live_child_walk() {
    isolated(
        "settings_tool_updates_keep_captured_live_child_walk",
        async |directory| {
            let cases: [(Edit, &[&str], &[&str]); 4] = [
                (
                    Edit::Append,
                    &["t0", "t1", "t2", "t3", "late"],
                    &["t0", "t1", "t2", "t3", "late"],
                ),
                (
                    Edit::RemoveWithNext,
                    &["t0", "t1", "t3"],
                    &["t0", "t2", "t3"],
                ),
                (Edit::Clear, &["t0", "t1", "t2", "t3"], &[]),
                (Edit::Replace, &["t0", "t1", "t2", "t3"], &["late"]),
            ];
            for query in ["images", "width"] {
                for (edit, visited, kept) in cases {
                    let (seen, held) = edited_update(directory, edit, query).await;
                    assert_eq!(seen, visited, "{query}: visits after {edit:?}");
                    assert_eq!(held, kept, "{query}: members after {edit:?}");
                }
            }
        },
    );
}

/// Whether the footer paints `label` in its current state.
fn footer_shows(world: &World, label: &str) -> bool {
    world
        .mode
        .footer
        .render(100)
        .iter()
        .any(|row| strip_ansi(row).unwrap().contains(label))
}

/// A one-pixel PNG as a retained tool result, carrying `text` beside the image.
fn image_result(text: &str) -> crate::ToolExecutionResult {
    let data = [
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/",
        "AAAAAAAABQABZHiVOAAAAABJRU5ErkJggg==",
    ]
    .concat();
    crate::ToolExecutionResult {
        content: vec![
            maestro_app::UserBlock::Text(maestro_app::TextContent {
                text: text.into(),
                text_signature: None,
            }),
            maestro_app::UserBlock::Image(maestro_app::ImageContent {
                data,
                mime_type: "image/png".into(),
            }),
        ],
        details: None,
        is_error: false,
    }
}

/// Adds `ids` as retained image-carrying tool rows over `world` and returns them.
fn retain_tools(world: &World, ids: &[&str]) -> Vec<Rc<crate::ToolExecutionComponent>> {
    ids.iter()
        .map(|id| {
            let tool = Rc::new(
                crate::ToolExecutionComponent::new(transcript::tool_params(world, id)).unwrap(),
            );
            tool.update_result(image_result(id), false).unwrap();
            world
                .mode
                .chat_container
                .add_child(Rc::clone(&tool) as Rc<dyn Component>);
            tool
        })
        .collect()
}

/// The rows a retained tool renders at `width`.
fn tool_rows(tool: &crate::ToolExecutionComponent, width: usize) -> Vec<String> {
    tool.render(width)
}

#[test]
fn settings_image_changes_reach_each_retained_tool_only() {
    isolated(
        "settings_image_changes_reach_each_retained_tool_only",
        async |directory| {
            let world = started_settings(directory).await;
            world
                .mode
                .tool_environment
                .terminal_image
                .set_capabilities(TerminalCapabilities {
                    images: Some(maestro_tui::ImageProtocol::Iterm2),
                    ..TerminalCapabilities::default()
                });
            let tools = retain_tools(&world, &["a", "b"]);
            let shown = tools
                .iter()
                .map(|tool| tool_rows(tool, 80).len())
                .collect::<Vec<_>>();
            let list = open(&world);
            choose(&list, "images", 0);
            assert!(
                !world
                    .host
                    .session()
                    .settings_manager()
                    .borrow()
                    .get_show_images()
            );
            for (tool, before) in tools.iter().zip(&shown) {
                assert!(
                    tool_rows(tool, 80).len() < *before,
                    "each retained image row follows the choice"
                );
            }
            choose(&list, "images", 0);
            for (tool, before) in tools.iter().zip(&shown) {
                assert_eq!(tool_rows(tool, 80).len(), *before);
            }
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_editor_options_reach_retained_default_editor() {
    isolated(
        "settings_editor_options_reach_retained_default_editor",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let editor = Rc::clone(&world.mode.editor);
            for padding in ["1", "2", "3", "0"] {
                let before = editor.render(60);
                search(&list, "padding");
                while !shows_value(&world, "Editor padding", padding) {
                    press(&list, "\r");
                }
                let after = editor.render(60);
                assert_ne!(before, after, "padding {padding} reaches the editor");
                assert_eq!(
                    world
                        .host
                        .session()
                        .settings_manager()
                        .borrow()
                        .get_editor_padding_x()
                        .to_string(),
                    padding
                );
            }
            for height in ["7", "10", "15", "20", "3", "5"] {
                search(&list, "Autocomplete");
                while !shows_value(&world, "Autocomplete max items", height) {
                    press(&list, "\r");
                }
                assert_eq!(
                    world
                        .host
                        .session()
                        .settings_manager()
                        .borrow()
                        .get_autocomplete_max_visible()
                        .to_string(),
                    height
                );
            }
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_command_does_not_consume_waiter_or_history() {
    isolated(
        "settings_command_does_not_consume_waiter_or_history",
        async |directory| {
            let world = started_settings(directory).await;
            let mut waiter = Box::pin(world.mode.get_user_input());
            assert_eq!(poll(&mut waiter), None);
            submit(&world.mode, "/settings");
            assert!(panel(&world).is_some());
            assert_eq!(poll(&mut waiter), None, "the waiter is untouched");
            assert!(recalled(&world.mode.editor).is_empty(), "no history entry");
            submit(&world.mode, "ordinary");
            assert_eq!(
                poll(&mut waiter),
                Some("ordinary".to_owned()),
                "the next submission reaches the same waiter"
            );
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_changes_flush_and_reopen_from_saved_preferences() {
    isolated(
        "settings_changes_flush_and_reopen_from_saved_preferences",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let settings = Rc::clone(world.host.session().settings_manager());
            choose(&list, "block", 0);
            choose(&list, "quiet", 0);
            choose(&list, "progress", 0);
            let expected = (
                settings.borrow().get_block_images(),
                settings.borrow().get_quiet_startup(),
                settings.borrow().get_show_terminal_progress(),
            );
            assert_eq!(expected, (true, true, true));
            let saved = settings.borrow().get_global_settings();
            let reopened = SettingsManager::in_memory(saved);
            assert!(reopened.get_block_images());
            assert!(reopened.get_quiet_startup());
            assert!(reopened.get_show_terminal_progress());
            assert_eq!(
                reopened.get_transport(),
                settings.borrow().get_transport(),
                "unrelated values keep their saved form"
            );
            drop(world.runtime.settle());
        },
    );
}

/// Registers a reasoning model of `provider` and makes it the session's model.
async fn use_reasoning_model(world: &World, provider: &str, id: &str) {
    let session = world.host.session();
    session
        .model_registry()
        .register_provider(
            provider,
            maestro_app::SharedHandle::new(std::sync::RwLock::new(
                maestro_app::ProviderConfigInput {
                    base_url: Some("https://example.invalid".into()),
                    api_key: Some("fixture".into()),
                    api: Some("openai-completions".into()),
                    models: Some(vec![maestro_app::ProviderModelInput {
                        reasoning: true,
                        ..model_input(id)
                    }]),
                    ..maestro_app::ProviderConfigInput::default()
                },
            )),
        )
        .unwrap();
    let model = session.model_registry().find(provider, id).unwrap();
    session.set_model(model).await.unwrap();
}

#[test]
fn settings_thinking_updates_session_footer_and_editor_border() {
    isolated(
        "settings_thinking_updates_session_footer_and_editor_border",
        async |directory| {
            let world = started_settings(directory).await;
            use_reasoning_model(&world, "reasoning", "deep").await;
            let session = world.host.session();
            let levels = session.get_available_thinking_levels();
            assert!(levels.len() > 1, "the model admits reasoning levels");
            let list = open(&world);
            choose(&list, "Thinking level", 0);
            press(&list, "x");
            press(&list, "\x1b[B");
            press(&list, "\r");
            let chosen = session.thinking_level();
            assert_ne!(chosen, maestro_app::ThinkingLevel::Off);
            let label = crate::components::thinking_selector::thinking_text(&chosen).0;
            assert_eq!(
                world
                    .host
                    .session()
                    .settings_manager()
                    .borrow()
                    .get_global_settings()
                    .0
                    .get("defaultThinkingLevel")
                    .and_then(|saved| saved.as_str()),
                Some(label)
            );
            let border = world.mode.editor.render(60).swap_remove(0);
            let theme = world.themes.theme().get().unwrap();
            let painted = theme.get_thinking_border_color(label)("─");
            let color = &painted[..painted.find('─').unwrap()];
            assert!(
                border.starts_with(color),
                "the border is painted for the level"
            );
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_thinking_visibility_rebuilds_retained_transcript() {
    isolated(
        "settings_thinking_visibility_rebuilds_retained_transcript",
        async |directory| {
            let world = started_settings(directory).await;
            let content = serde_json::json!([
                {"type": "thinking", "thinking": "CONSIDERED"},
                {"type": "text", "text": "ANSWER"}
            ]);
            append_messages(
                &world,
                vec![
                    transcript::user_record("ASK"),
                    transcript::assistant(&content, "stop", None),
                ],
            );
            world.mode.render_initial_messages().unwrap();
            let before = world.mode.chat_container.render(100);
            assert!(before.iter().any(|row| row.contains("CONSIDERED")));
            let list = open(&world);
            choose(&list, "Hide", 0);
            assert!(
                world
                    .host
                    .session()
                    .settings_manager()
                    .borrow()
                    .get_hide_thinking_block()
            );
            assert!(
                world.mode.hide_thinking_block.get(),
                "the mode keeps the retained flag"
            );
            let after = world.mode.chat_container.render(100);
            assert!(
                !after.iter().any(|row| row.contains("CONSIDERED")),
                "the rebuilt transcript hides thinking"
            );
            assert!(after.iter().any(|row| row.contains("ANSWER")));
            drop(world.runtime.settle());
        },
    );
}

/// Appends `messages` to the session of `world`.
fn append_messages(world: &World, messages: Vec<serde_json::Value>) {
    let session = world.host.session();
    let mut manager = session.session_manager().borrow_mut();
    for message in messages {
        manager.append_message(message).unwrap();
    }
}

#[test]
fn settings_skill_commands_refresh_registered_autocomplete() {
    isolated(
        "settings_skill_commands_refresh_registered_autocomplete",
        async |directory| {
            let world = Setup {
                directories: Some(registry(directory)),
                skills: vec![skill("helper")],
                ..Setup::default()
            }
            .build(directory);
            start(&world).await;
            let session = world.host.session();
            let commands = session.extension_runner().get_registered_commands();
            // Built while skills are enabled, so it keeps offering the skill once disabled.
            let stale = world.mode.create_base_autocomplete_provider(&commands);
            choose(&open(&world), "skill", 0);
            press(&open_list(&world), "\x1b");
            assert!(panel(&world).is_none(), "the editor is shown again");
            assert!(
                !session
                    .settings_manager()
                    .borrow()
                    .get_enable_skill_commands()
            );
            assert_eq!(completed(&world).await, "/skill:hel");
            world.mode.editor.set_autocomplete_provider(stale);
            assert_eq!(
                completed(&world).await,
                "/skill:helper ",
                "a provider kept from before the change still completes the skill"
            );
            world.mode.setup_autocomplete_provider().unwrap();
            assert_eq!(completed(&world).await, "/skill:hel");
            choose(&open(&world), "skill", 0);
            press(&open_list(&world), "\x1b");
            assert!(panel(&world).is_none(), "the editor is shown again");
            assert!(
                session
                    .settings_manager()
                    .borrow()
                    .get_enable_skill_commands()
            );
            assert_eq!(
                completed(&world).await,
                "/skill:helper ",
                "the rebuilt provider offers the loaded skill command again"
            );
            drop(world.runtime.settle());
        },
    );
}

/// The retained editor's text after it holds `/skill:hel` and the terminal sends Tab twice:
/// the first Tab opens the completion menu the installed provider answers, the second
/// confirms its selection.
async fn completed(world: &World) -> String {
    world.mode.editor.set_text("/skill:hel");
    for _ in 0..2 {
        world.terminal.send_input("\t");
        for request in world.runtime.take_futures() {
            request.await.unwrap();
        }
    }
    world.mode.editor.get_text()
}

/// A loaded skill named `name`.
fn skill(name: &str) -> maestro_app::Skill {
    maestro_app::Skill {
        name: name.to_owned(),
        description: format!("{name} skill"),
        file_path: format!("/skills/{name}/SKILL.md"),
        base_dir: "/skills".to_owned(),
        source_info: maestro_app::SourceInfo {
            path: format!("/skills/{name}/SKILL.md"),
            base_dir: Some("/skills".to_owned()),
            source: format!("skill:{name}"),
            scope: maestro_app::SourceScope::User,
            origin: maestro_app::SourceOrigin::TopLevel,
        },
        disable_model_invocation: false,
    }
}

#[test]
fn settings_callback_teardown_releases_mode() {
    isolated(
        "settings_callback_teardown_releases_mode",
        async |directory| {
            let (weak, list, callbacks) = {
                let world = started_settings(directory).await;
                let list = open(&world);
                let callbacks = world.mode.settings_callbacks(Rc::new(|| {}));
                (Rc::downgrade(&world.mode), list, callbacks)
            };
            assert!(weak.upgrade().is_none(), "the retained list keeps no mode");
            (callbacks.on_show_images_change)(true);
            (callbacks.on_cancel)();
            assert!(
                weak.upgrade().is_none(),
                "released callbacks resurrect nothing"
            );
            drop(list);
            drop(callbacks);
        },
    );
}

/// Every typed preference the panel reads, as text.
fn typed_preferences(world: &World) -> Vec<String> {
    let session = world.host.session();
    let settings = session.settings_manager().borrow();
    vec![
        settings.get_show_images().to_string(),
        settings.get_image_width_cells().to_string(),
        settings.get_image_auto_resize().to_string(),
        settings.get_block_images().to_string(),
        settings.get_enable_skill_commands().to_string(),
        settings.get_transport().as_str().to_owned(),
        settings.get_theme().unwrap_or_default(),
        settings.get_collapse_changelog().to_string(),
        settings.get_enable_install_telemetry().to_string(),
        settings.get_double_escape_action().as_str().to_owned(),
        settings.get_tree_filter_mode().as_str().to_owned(),
        settings.get_show_hardware_cursor().to_string(),
        settings.get_editor_padding_x().to_string(),
        settings.get_autocomplete_max_visible().to_string(),
        settings.get_quiet_startup().to_string(),
        settings.get_clear_on_shrink().to_string(),
        settings.get_show_terminal_progress().to_string(),
        format!("{:?}", settings.get_warnings().0),
    ]
}

/// A started world whose supplied settings document is `raw`, applied after startup so
/// the startup changelog save cannot discard it.
async fn typed_world(directory: &Path, raw: serde_json::Value) -> World {
    let supplied: maestro_app::Settings = serde_json::from_value(raw).unwrap();
    let world = started_settings(directory).await;
    world
        .host
        .session()
        .settings_manager()
        .borrow_mut()
        .apply_overrides(supplied);
    world
}

/// Every read that keeps its stored form regardless of the class, for equality checks.
fn comparable(values: &[String]) -> Vec<String> {
    [&values[..1], &values[2..5], &values[7..12], &values[14..17]].concat()
}

/// The text a supplied number root key yields, or its default.
fn supplied_number(raw: &serde_json::Value, key: &str, fallback: &str) -> String {
    match raw.get(key).and_then(serde_json::Value::as_f64) {
        Some(value) => value.to_string(),
        None => fallback.to_owned(),
    }
}

/// The theme name `raw` leaves stored.
fn supplied_theme(raw: &serde_json::Value) -> String {
    match raw.get("theme") {
        Some(serde_json::Value::String(name)) => name.clone(),
        Some(_) => String::new(),
        None => "dark".to_owned(),
    }
}

/// Asserts the boolean reads one supplied document produces.
fn assert_boolean_class(raw: &serde_json::Value, read: &[String], defaults: &[String]) {
    let written_out = raw
        .get("terminal")
        .and_then(|terminal| terminal.get("showImages"))
        == Some(&serde_json::Value::Bool(false));
    if !written_out {
        assert_eq!(
            comparable(read),
            comparable(defaults),
            "a wrong JSON class is unset: {raw}"
        );
        return;
    }
    assert!(read[0] == "false" && read[3] == "false" && read[4] == "false");
    assert!(read[7] == "false" && read[8] == "false" && read[11] == "false");
    assert!(read[15] == "false" && read[16] == "false");
}

/// Asserts one supplied JSON document against the typed reads it produces.
async fn assert_class(
    directory: &Path,
    raw: &serde_json::Value,
    defaults: &[String],
    stored: &serde_json::Map<String, serde_json::Value>,
) {
    let world = typed_world(directory, raw.clone()).await;
    let read = typed_preferences(&world);
    assert_boolean_class(raw, &read, defaults);
    let transport = match raw.get("transport") {
        Some(serde_json::Value::String(name)) => name.clone(),
        _ => defaults[5].clone(),
    };
    assert_eq!(
        read[5], transport,
        "an unknown stored transport keeps its text"
    );
    assert_eq!(
        read[6],
        supplied_theme(raw),
        "the stored theme is read as written"
    );
    let width = match raw
        .get("terminal")
        .and_then(|terminal| terminal.get("imageWidthCells"))
        .and_then(serde_json::Value::as_f64)
    {
        Some(value) => f64::max(1.0, value.floor()).to_string(),
        None => defaults[1].clone(),
    };
    assert_eq!(
        read[1], width,
        "a finite width is floored and kept at least 1"
    );
    assert_eq!(
        read[12],
        supplied_number(raw, "editorPaddingX", &defaults[12]),
        "a numeric padding is read as a number"
    );
    assert_eq!(
        read[13],
        supplied_number(raw, "autocompleteMaxVisible", &defaults[13]),
        "a numeric completion height is read as a number"
    );
    let warnings = match raw.get("warnings") {
        Some(serde_json::Value::Object(members)) => {
            assert_eq!(members.len(), 2, "supplied warning members are kept");
            read[17].clone()
        }
        _ => defaults[17].clone(),
    };
    assert_eq!(
        read[17], warnings,
        "a supplied warning object keeps its members"
    );
    assert_stored(&world, stored);
}

/// Asserts the panel still reaches the inventory and the reads rewrote no stored setting.
fn assert_stored(world: &World, stored: &serde_json::Map<String, serde_json::Value>) {
    assert!(
        world.mode.settings_config().unwrap().available_themes.len() >= 2,
        "the panel still reaches the real inventory"
    );
    assert_eq!(
        Rc::clone(world.host.session().settings_manager())
            .borrow()
            .get_global_settings()
            .0,
        *stored,
        "a typed read never rewrites the stored settings"
    );
}

/// The seven JSON classes the typed reads are exercised over.
fn supplied_classes() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({}),
        serde_json::json!({"terminal": {"showImages": null, "clearOnShrink": null},
            "transport": null, "theme": null, "warnings": null}),
        serde_json::json!({"terminal": {"showImages": "authored",
            "imageWidthCells": "authored", "clearOnShrink": null},
            "transport": "authored", "theme": "authored", "editorPaddingX": "authored"}),
        serde_json::json!({"terminal": {"showImages": 2.75, "imageWidthCells": 2.75,
            "clearOnShrink": null}, "editorPaddingX": 2.75,
            "autocompleteMaxVisible": 2.75}),
        serde_json::json!({"terminal": {"showImages": false, "clearOnShrink": false},
            "images": {"autoResize": false, "blockImages": false},
            "enableSkillCommands": false, "collapseChangelog": false,
            "enableInstallTelemetry": false, "showHardwareCursor": false,
            "quietStartup": false, "showTerminalProgress": false}),
        serde_json::json!({"warnings": {"anthropicExtraUsage": false,
            "unknownMember": true}, "terminal": {"showImages": [2]},
            "theme": {"nested": true}}),
        serde_json::json!({"terminal": {"showImages": ["x", false]},
            "images": ["x", false], "warnings": [false]}),
    ]
}

#[test]
fn settings_snapshot_uses_typed_preferences_without_rewriting_raw_settings() {
    isolated(
        "settings_snapshot_uses_typed_preferences_without_rewriting_raw_settings",
        async |directory| {
            let baseline = started_settings(directory).await;
            let defaults = typed_preferences(&baseline);
            let stored = Rc::clone(baseline.host.session().settings_manager())
                .borrow()
                .get_global_settings()
                .0;
            for raw in supplied_classes() {
                assert_class(directory, &raw, &defaults, &stored).await;
            }
            for width in [-1.0, 0.75] {
                let world = typed_world(
                    directory,
                    serde_json::json!({"terminal": {"imageWidthCells": width}}),
                )
                .await;
                let read = Rc::clone(world.host.session().settings_manager())
                    .borrow()
                    .get_image_width_cells();
                assert!(read.total_cmp(&1.0).is_eq(), "width {width} is raised to 1");
            }
        },
    );
}

#[test]
fn settings_infallible_callbacks_report_original_failures() {
    isolated(
        "settings_infallible_callbacks_report_original_failures",
        async |directory| {
            let world = started_settings(directory).await;
            let list = open(&world);
            let settings = Rc::clone(world.host.session().settings_manager());
            assert!(
                !settings.borrow().get_show_hardware_cursor(),
                "the child starts without the hardware cursor"
            );
            choose(&list, "hardware", 0);
            assert!(settings.borrow().get_show_hardware_cursor());
            assert!(
                world.runtime.drain_futures().is_empty(),
                "the control succeeds"
            );
            world
                .terminal
                .set_hide_error(Some(std::io::ErrorKind::PermissionDenied));
            choose(&list, "hardware", 0);
            assert!(
                !settings.borrow().get_show_hardware_cursor(),
                "the preference is written before the terminal effect fails"
            );
            let errors = world.runtime.drain_futures();
            assert_eq!(errors.len(), 1, "one original error reaches the runtime");
            assert_eq!(
                errors[0]
                    .downcast_ref::<std::io::Error>()
                    .map(std::io::Error::kind),
                Some(std::io::ErrorKind::PermissionDenied),
                "the original error keeps its kind"
            );
            world.terminal.set_hide_error(None);
            choose(&list, "hardware", 0);
            assert!(world.runtime.drain_futures().is_empty(), "control succeeds");
            drop(world.runtime.settle());
        },
    );
}

/// The published theme's name, or its absence.
fn published(world: &World) -> Option<String> {
    world
        .themes
        .theme()
        .get()
        .ok()
        .and_then(|theme| theme.name().map(str::to_owned))
}

/// Writes a custom theme file named `name` holding `document`.
fn custom_theme(directory: &Path, name: &str, document: &str) {
    let custom = directory.join("themes");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(custom.join(format!("{name}.json")), document).unwrap();
}

#[test]
fn settings_theme_preview_commit_and_failure_keep_distinct_effects() {
    isolated(
        "settings_theme_preview_commit_and_failure_keep_distinct_effects",
        async |directory| {
            custom_theme(directory, "broken", "{ not json");
            let world = started_settings(directory).await;
            let settings = Rc::clone(world.host.session().settings_manager());
            let names = world.themes.get_available_themes().unwrap();
            assert!(names.contains(&"broken".to_owned()), "{names:?}");
            let list = open(&world);
            choose(&list, "Theme", 0);
            press(&list, "\x1b[B");
            assert_eq!(
                published(&world).as_deref(),
                Some("light"),
                "preview paints"
            );
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("dark"),
                "preview never saves"
            );
            press(&list, "\r");
            assert_eq!(settings.borrow().get_theme().as_deref(), Some("light"));
            choose(&list, "Theme", 0);
            let index = names.iter().position(|name| name == "broken").unwrap();
            let committed = names.iter().position(|name| name == "light").unwrap();
            for _ in 0..(index + names.len() - committed) % names.len() {
                press(&list, "\x1b[B");
            }
            press(&list, "\r");
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("broken"),
                "commit saves the requested name"
            );
            assert_eq!(published(&world).as_deref(), Some("dark"), "it falls back");
            let rows = world.mode.chat_container.render(100).join("\n");
            assert!(rows.contains("Failed to load theme \"broken\""), "{rows}");
            assert!(rows.contains("Fell back to dark theme."));
            drop(world.runtime.settle());
        },
    );
}

/// The shipped dark theme, renamed `name` with its colours reading from `accent`.
fn theme_document(name: &str, accent: &str) -> String {
    std::fs::read_to_string(format!("{SHIPPED}/dark.json"))
        .unwrap()
        .replace("\"dark\"", &format!("\"{name}\""))
        .replace("#D9A066", accent)
}

/// A started mode whose panel reads the theme inventory and whose watches reach `watched`.
async fn watching_settings(directory: &Path, watched: &Rc<super::Watched>) -> World {
    let world = Setup {
        directories: Some(registry(directory)),
        watcher: Some(Rc::clone(watched) as Rc<dyn FooterOperations>),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    world.runtime.settle().unwrap();
    world
}

/// Move the open theme submenu from the saved `dark` row to `watched`, previewing it.
fn preview_watched(world: &World, list: &SettingsList) {
    choose(list, "Theme", 0);
    let names = world.themes.get_available_themes().unwrap();
    let previewed = names.iter().position(|name| name == "watched").unwrap();
    let saved = names.iter().position(|name| name == "dark").unwrap();
    for _ in 0..(previewed + names.len() - saved) % names.len() {
        press(list, "\x1b[B");
    }
    assert_eq!(
        published(world).as_deref(),
        Some("watched"),
        "the preview publishes the selection"
    );
}

/// Rewrite the watched custom theme as `name` with `accent`, then announce and reload it.
fn edit_watched_theme(directory: &Path, watched: &super::Watched, name: &str, accent: &str) {
    custom_theme(directory, "watched", &theme_document(name, accent));
    watched.announce(Some("watched.json"));
    watched.reload();
}

/// Settle the frame the theme change requests and check it paints the published accent.
fn reloaded_frame(world: &World) {
    assert_eq!(
        world.runtime.pending(),
        1,
        "the theme change requests exactly one frame"
    );
    world.terminal.clear_writes();
    world.runtime.settle().unwrap();
    let theme = world.themes.theme().get().unwrap();
    let accent = theme.get_fg_ansi(&ThemeColor::Accent).unwrap();
    let frame = world.terminal.writes().concat();
    assert!(frame.contains(accent), "the published accent: {frame:?}");
}

#[test]
fn settings_theme_preview_reloads_the_edited_file() {
    isolated(
        "settings_theme_preview_reloads_the_edited_file",
        async |directory| {
            custom_theme(directory, "watched", &theme_document("watched", "#010203"));
            let watched = Rc::new(super::Watched::default());
            let world = watching_settings(directory, &watched).await;
            let settings = Rc::clone(world.host.session().settings_manager());
            let list = open(&world);
            preview_watched(&world, &list);
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("dark"),
                "the preview saves nothing"
            );
            assert!(!watched.open.borrow()[0].get(), "the preview watch is open");
            edit_watched_theme(directory, &watched, "edited", "#204060");
            assert_eq!(
                published(&world).as_deref(),
                Some("edited"),
                "the preview reloads the edited file"
            );
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("dark"),
                "the reload saves nothing"
            );
            reloaded_frame(&world);
        },
    );
}

#[test]
fn settings_theme_commit_reloads_a_later_edit() {
    isolated(
        "settings_theme_commit_reloads_a_later_edit",
        async |directory| {
            custom_theme(directory, "watched", &theme_document("watched", "#010203"));
            let watched = Rc::new(super::Watched::default());
            let world = watching_settings(directory, &watched).await;
            let settings = Rc::clone(world.host.session().settings_manager());
            let list = open(&world);
            preview_watched(&world, &list);
            press(&list, "\r");
            assert_eq!(settings.borrow().get_theme().as_deref(), Some("watched"));
            world.runtime.settle().unwrap();
            assert_eq!(
                watched.open.borrow().len(),
                2,
                "the committed selection starts its own watch"
            );
            assert!(watched.open.borrow()[0].get(), "the preview watch closed");
            assert!(
                !watched.open.borrow()[1].get(),
                "the latest watch waits for its edit"
            );
            edit_watched_theme(directory, &watched, "edited-after-commit", "#406080");
            assert_eq!(published(&world).as_deref(), Some("edited-after-commit"));
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("watched"),
                "the committed preference stays saved"
            );
            reloaded_frame(&world);
        },
    );
}

#[test]
fn settings_theme_cancel_replays_the_saved_theme_without_persisting() {
    isolated(
        "settings_theme_cancel_replays_the_saved_theme_without_persisting",
        async |directory| {
            let world = settings_world(directory, |settings| {
                settings.set_theme("light".to_owned());
            })
            .await;
            let settings = Rc::clone(world.host.session().settings_manager());
            assert_eq!(published(&world).as_deref(), Some("light"));
            let list = open(&world);
            choose(&list, "Theme", 0);
            press(&list, "\x1b[B");
            assert_eq!(published(&world).as_deref(), Some("dark"));
            press(&list, "\x1b");
            assert!(matches!(published(&world).as_deref(), Some("light")));
            assert_eq!(settings.borrow().get_theme().as_deref(), Some("light"));
            choose(&list, "Theme", 0);
            press(&list, "\x1b[B");
            press(&list, "\r");
            assert_eq!(settings.borrow().get_theme().as_deref(), Some("dark"));
            choose(&list, "Theme", 0);
            press(&list, "\x1b[B");
            press(&list, "\x1b");
            assert_eq!(
                published(&world).as_deref(),
                Some("dark"),
                "cancel replays the latest committed selection"
            );
            drop(world.runtime.settle());
        },
    );
}

/// Refuses explicit behaviour edits while allowing tool refresh candidates.
struct RefuseBehaviour;
impl maestro_app::SettingsDecider for RefuseBehaviour {
    fn decide(&self, request: &maestro_app::DecisionRequest<'_>) -> Result<(), String> {
        if matches!(request, maestro_app::DecisionRequest::SettingsEdit { .. }) {
            return Err("release refuses behaviour changes".into());
        }
        Ok(())
    }
}

/// A refused transport selection cannot escape through the live agent field.
#[test]
fn refused_transport_selection_preserves_live_agent_and_settings() {
    isolated(
        "refused_transport_selection_preserves_live_agent_and_settings",
        async |directory| {
            let world = started_settings(directory).await;
            let session = world.host.session();
            let before = session.agent().transport.read().unwrap().clone();
            session
                .settings_manager()
                .borrow_mut()
                .register_decider(std::sync::Arc::new(RefuseBehaviour))
                .unwrap();
            let list = open(&world);
            choose(&list, "Transport", 0);
            assert_eq!(*session.agent().transport.read().unwrap(), before);
            assert_eq!(
                session.settings_manager().borrow().get_transport(),
                saved(&before)
            );
            drop(world.runtime.settle());
        },
    );
}
