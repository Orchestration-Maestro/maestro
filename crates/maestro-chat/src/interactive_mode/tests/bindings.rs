//! Binding the current session's extensions, settings and errors to the retained mode.
use super::{Setup, World, cases, isolated, roles, started};
use crate::interactive_mode::InteractiveMode;
use maestro_app::{
    DiagnosticErrorInfo, ExtensionError, ExtensionFactory, ExtensionShortcut, LoadExtensionsResult,
    Local, ReadonlyFooterDataProvider, ReadonlySessionManager, ResourcesDiscoverEvent,
    ResourcesDiscoverResult, SessionStartEvent, strip_ansi,
};
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use maestro_tui::{Component, tui::InputHandler};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::{Rc, Weak},
    sync::RwLock,
};

/// Every recorded binding case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_bindings.json");

/// The recorded operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// Runtime settings applied to the presentation.
    #[serde(rename_all = "camelCase")]
    Settings {
        /// Editor padding.
        padding: u32,
        /// Visible completion rows.
        max: u32,
        /// Whether the hardware cursor is shown.
        hardware: bool,
        /// Whether shrinking content clears the screen.
        clear: bool,
        /// Whether thinking is hidden.
        hidden: bool,
        /// Whether automatic compaction is enabled.
        compact: bool,
        /// Whether hiding the hardware cursor fails.
        #[serde(default)]
        fail_hardware: bool,
    },
    /// One rebinding, with or without a previous subscription.
    Rebind {
        /// Whether a subscription is installed before rebinding.
        subscription: bool,
        /// The phase that fails, when one does.
        fail: Option<String>,
    },
    /// One binding of the current session.
    Binding {
        /// The phase that fails, when one does.
        fail: Option<String>,
    },
    /// A shutdown request through a bound context.
    BoundShutdown {
        /// Whether the session streams when the request arrives.
        streaming: bool,
        /// Whether a request was already recorded.
        before: bool,
    },
    /// A shutdown request whose streaming state changes after the context is created.
    BoundShutdownTransition {
        /// Whether the session streamed when the context was created.
        before: bool,
        /// Whether the session streams when the request arrives.
        after: bool,
    },
    /// An attributed failure the runner reports to the bound listener.
    BoundError {
        /// The supplied stack.
        stack: Option<String>,
    },
    /// An attributed failure shown in the transcript.
    ExtensionError {
        /// The supplied stack.
        stack: Option<String>,
    },
}

/// Settings effects reached, the thinking visibility and the failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settled {
    /// The effects reached, by name or `[name, value]`, in order.
    calls: Vec<Value>,
    /// The thinking visibility afterwards.
    hidden: bool,
    /// The failure, when one stopped the effects.
    #[serde(default)]
    error: Option<String>,
}

/// Phases reached in order and the failure.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Phases {
    /// The phases reached, in order.
    calls: Vec<String>,
    /// Whether the previous subscription was cleared.
    #[serde(default)]
    unsubscribe_cleared: Option<bool>,
    /// Whether the started UI reached the extensions.
    #[serde(default)]
    ui_forwarded: Option<bool>,
    /// The failure, when one stopped the phases.
    #[serde(default)]
    error: Option<String>,
}

/// Whether the bound handler started a shutdown and the recorded request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requested {
    /// `shutdown` when the handler started one.
    calls: Vec<String>,
    /// The request flag afterwards.
    requested: bool,
}

/// The failures the bound listener received.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Forwarded {
    /// The failures, in order.
    calls: Vec<Attributed>,
}

/// One attributed failure.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Attributed {
    /// The extension's path.
    extension_path: String,
    /// The failure message.
    error: String,
    /// The supplied stack.
    #[serde(default)]
    stack: Option<String>,
}

/// The transcript rows of a shown failure.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Shown {
    /// Rendered rows at width 80, role markers in place of colours.
    lines: Vec<String>,
    /// Each added text, role markers in place of colours.
    texts: Vec<String>,
    /// Children added to the transcript.
    children: usize,
    /// Whether a frame was requested.
    render_requested: bool,
}

/// The recorded cases of `test`, each observation read as `E`.
fn recorded<E: DeserializeOwned>(test: &str) -> Vec<(Input, E)> {
    cases::<Input, Value>(FIXTURES, test)
        .into_iter()
        .map(|(input, expected)| (input, serde_json::from_value(expected).unwrap()))
        .collect()
}

/// An extension factory that registers nothing.
fn idle() -> ExtensionFactory<Local> {
    Rc::new(|_| Box::pin(std::future::ready(Ok(()))))
}

/// A theme without colours, so every coloured row fails.
fn colourless(world: &World) {
    let empty = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
    world.themes.set_theme_instance(Rc::new(empty)).unwrap();
}

/// The current session's settings, written as `input` selects.
fn write_settings(world: &World, input: &Input) {
    let Input::Settings {
        padding,
        max,
        hardware,
        clear,
        hidden,
        compact,
        ..
    } = *input
    else {
        panic!("a settings case");
    };
    let session = world.mode.runtime_host.session();
    {
        let mut settings = session.settings_manager().borrow_mut();
        settings.set_editor_padding_x(f64::from(padding));
        settings.set_autocomplete_max_visible(f64::from(max));
        settings.set_show_hardware_cursor(hardware);
        settings.set_clear_on_shrink(clear);
        settings.set_hide_thinking_block(hidden);
    }
    session.set_auto_compaction_enabled(compact);
}

/// Whether the footer shows the automatic-compaction marker.
fn auto_marker(world: &World) -> bool {
    world
        .mode
        .footer
        .render(120)
        .iter()
        .any(|row| strip_ansi(row).unwrap().contains("(auto)"))
}

/// Counts branch-change notifications, which the footer metadata sends on each new cwd.
fn cwd_changes(world: &World) -> Rc<Cell<usize>> {
    let changes = Rc::new(Cell::new(0));
    let counted = Rc::clone(&changes);
    drop(
        world
            .mode
            .footer_data_provider
            .on_branch_change(Rc::new(move || counted.set(counted.get() + 1))),
    );
    changes
}

/// The effect names a settings case reached.
fn reached(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .map(|call| call.as_str().or_else(|| call[0].as_str()).unwrap())
        .collect()
}

/// Asserts the editor's padding and visible completion rows.
fn assert_editor(world: &World, padding: u32, max: u32) {
    let editor = &world.mode.editor;
    assert_eq!(
        editor.get_padding_x().to_bits(),
        f64::from(padding).to_bits()
    );
    assert_eq!(
        editor.get_autocomplete_max_visible(),
        usize::try_from(max).unwrap()
    );
}

/// Asserts the settings effects `expected` reached for `input`, and only those; the
/// effects after a failure keep the `before` clear-on-shrink, padding and row values.
fn assert_settings(world: &World, input: &Input, expected: &Settled, before: (bool, u32, u32)) {
    let Input::Settings {
        padding,
        max,
        hardware,
        clear,
        compact,
        ..
    } = *input
    else {
        panic!("a settings case");
    };
    let reached = reached(&expected.calls);
    assert_eq!(world.mode.hide_thinking_block.get(), expected.hidden);
    assert_eq!(auto_marker(world), compact, "{reached:?}");
    assert_eq!(world.mode.ui.get_show_hardware_cursor(), hardware);
    let (clear, padding, max) = if reached.contains(&"max") {
        (clear, padding, max)
    } else {
        before
    };
    assert_eq!(world.mode.ui.get_clear_on_shrink(), clear);
    assert_editor(world, padding, max);
}

#[test]
fn runtime_settings_follow_the_current_session_without_crossing_policies() {
    const NAME: &str = "runtime_settings_follow_the_current_session_without_crossing_policies";
    isolated(NAME, async |directory| {
        let world = started(directory).await;
        let changes = cwd_changes(&world);
        for (input, expected) in recorded::<Settled>(NAME) {
            assert!(expected.error.is_none());
            assert_eq!(
                reached(&expected.calls),
                [
                    "session", "compact", "cwd", "hardware", "clear", "padding", "max"
                ]
            );
            let hardware_before = world.mode.ui.get_show_hardware_cursor();
            write_settings(&world, &input);
            world
                .mode
                .footer_data_provider
                .set_cwd("/elsewhere".to_owned());
            let moved = changes.get();
            world.terminal.clear_writes();
            world.mode.apply_runtime_settings().unwrap();
            assert_eq!(changes.get(), moved + 1, "the session's cwd comes back");
            assert_settings(&world, &input, &expected, (false, 0, 0));
            let Input::Settings { hardware, .. } = input else {
                unreachable!()
            };
            let hidden = world.terminal.writes().iter().any(|w| w == "\x1b[?25l");
            assert_eq!(hidden, hardware_before && !hardware);
            world.mode.apply_runtime_settings().unwrap();
            assert_eq!(changes.get(), moved + 1, "an unchanged cwd notifies nobody");
        }
    });
}

#[test]
fn runtime_settings_propagate_hardware_cursor_errors() {
    const NAME: &str = "runtime_settings_propagate_hardware_cursor_errors";
    isolated(NAME, async |directory| {
        for (input, expected) in recorded::<Settled>(NAME) {
            assert!(matches!(
                input,
                Input::Settings {
                    fail_hardware: true,
                    ..
                }
            ));
            let world = Setup {
                configure: Box::new(|settings| {
                    settings.set_show_hardware_cursor(true);
                    settings.set_clear_on_shrink(false);
                    settings.set_editor_padding_x(2.0);
                    settings.set_autocomplete_max_visible(9.0);
                }),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            write_settings(&world, &input);
            world
                .terminal
                .set_hide_error(Some(std::io::ErrorKind::PermissionDenied));
            let error = world.mode.apply_runtime_settings().unwrap_err();
            assert!(expected.error.is_some());
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            assert_settings(&world, &input, &expected, (false, 2, 9));

            world.terminal.set_hide_error(None);
            world.terminal.clear_writes();
            world.mode.apply_runtime_settings().unwrap();
            assert!(
                !world.terminal.writes().iter().any(|w| w == "\x1b[?25l"),
                "the reached cursor state is kept, not written again"
            );
            let all = Settled {
                calls: vec![json!("max")],
                hidden: expected.hidden,
                error: None,
            };
            assert_settings(&world, &input, &all, (false, 2, 9));
        }
    });
}

/// Types `/` into the editor, waits for the command list and returns the editor's plain
/// rows, then closes the list.
async fn command_rows(world: &World) -> Vec<String> {
    let editor = &world.mode.editor;
    editor.set_text("");
    editor.handle_input("/");
    for _ in 0..10_000 {
        world.runtime.settle().unwrap();
        assert!(world.runtime.drain_futures().is_empty());
        if editor.is_showing_autocomplete() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(editor.is_showing_autocomplete(), "the command list opens");
    let rows = editor
        .render(80)
        .iter()
        .map(|row| strip_ansi(row).unwrap())
        .collect();
    editor.handle_input("\x1b");
    editor.set_text("");
    rows
}

#[test]
fn runtime_settings_update_the_default_editor_once() {
    isolated(
        "runtime_settings_update_the_default_editor_once",
        async |directory| {
            let world = started(directory).await;
            for (padding, max) in [(2, 9), (0, 5), (0, 3), (2, 3)] {
                {
                    let session = world.mode.runtime_host.session();
                    let mut settings = session.settings_manager().borrow_mut();
                    settings.set_editor_padding_x(f64::from(padding));
                    settings.set_autocomplete_max_visible(f64::from(max));
                }
                world.runtime.settle().unwrap();
                world.mode.apply_runtime_settings().unwrap();
                assert_editor(&world, padding, max);
                world.mode.editor.set_text("x");
                let content = strip_ansi(&world.mode.editor.render(40)[1]).unwrap();
                let indent = " ".repeat(usize::try_from(padding).unwrap());
                assert!(content.starts_with(&format!("{indent}x")), "{content:?}");
                // Below the editor's closing border: one row per visible command, then the
                // scroll position.
                let rows = command_rows(&world).await;
                let commands = rows[3..]
                    .iter()
                    .filter(|row| !row.trim_start().starts_with('('))
                    .count();
                assert_eq!(
                    commands,
                    usize::try_from(max).unwrap(),
                    "visible command rows: {rows:#?}"
                );
            }
        },
    );
}

/// Observations an extension makes while the session starts.
#[derive(Default)]
struct Probe {
    /// The mode the extension observes, once built.
    mode: RefCell<Weak<InteractiveMode>>,
    /// One record per session start.
    seen: RefCell<Vec<Value>>,
    /// Times a wrapped subscription removal ran.
    removed: Cell<usize>,
    /// Whether the next session start fails.
    fail: Cell<bool>,
}

/// An extension that records `probe`'s observations on each session start, fails it on
/// request, and registers the `ctrl+q` shortcut.
fn probing(probe: &Rc<Probe>) -> ExtensionFactory<Local> {
    let probe = Rc::clone(probe);
    Rc::new(move |api| {
        let probe = Rc::clone(&probe);
        let registered = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, context| {
                let ui = context.has_ui().unwrap();
                Box::pin(std::future::ready(observe_start(&probe, ui)))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// Records what `probe`'s mode shows at one session start, failing it on request.
fn observe_start(probe: &Probe, ui: bool) -> Result<Option<()>, maestro_app::EventError> {
    if let Some(mode) = probe.mode.borrow().upgrade() {
        let session = mode.runtime_host.session();
        probe.seen.borrow_mut().push(json!({
            "ui": ui,
            "removed": probe.removed.get(),
            "hidden": mode.hide_thinking_block.get(),
            "subscribed": mode.unsubscribe.borrow().is_some(),
            "settingsFree": session.settings_manager().try_borrow_mut().is_ok(),
        }));
    }
    if probe.fail.get() {
        return Err(Box::new(DiagnosticErrorInfo::new(None, "start refused")));
    }
    Ok(None)
}

/// A started mode whose extension `probe` observes each session start.
async fn probed(directory: &Path) -> (World, Rc<Probe>) {
    let probe = Rc::new(Probe::default());
    let world = Setup {
        extension: Some(probing(&probe)),
        ..Setup::default()
    }
    .build(directory);
    *probe.mode.borrow_mut() = Rc::downgrade(&world.mode);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    (world, probe)
}

/// Replaces the installed subscription with one whose removal `probe` counts, or removes
/// it, as `present` selects.
fn prepare_subscription(world: &World, probe: &Rc<Probe>, present: bool) {
    let real = world.mode.unsubscribe.take().unwrap();
    if present {
        let counted = Rc::clone(probe);
        world.mode.unsubscribe.replace(Some(Box::new(move || {
            counted.removed.set(counted.removed.get() + 1);
            real();
        })));
    } else {
        real();
    }
}

/// Records each terminal title the mode writes.
fn titles(world: &World) -> Rc<RefCell<Vec<String>>> {
    let titles = Rc::new(RefCell::new(Vec::new()));
    let written = Rc::clone(&titles);
    world.terminal.set_tap(move |bytes| {
        if let Some(title) = bytes.strip_prefix("\x1b]0;") {
            written
                .borrow_mut()
                .push(title.trim_end_matches('\x07').to_owned());
        }
    });
    titles
}

#[test]
fn session_rebinding_unsubscribes_before_settings_binding_and_subscription() {
    const NAME: &str = "session_rebinding_unsubscribes_before_settings_binding_and_subscription";
    isolated(NAME, async |directory| {
        let (world, probe) = probed(directory).await;
        assert_eq!(probe.seen.borrow().len(), 1, "startup binds once");
        assert_eq!(
            probe.seen.borrow()[0]["subscribed"],
            json!(false),
            "startup subscribes after binding"
        );
        assert!(world.mode.unsubscribe.borrow().is_some());
        for (input, expected) in recorded::<Phases>(NAME) {
            let Input::Rebind {
                subscription,
                fail: None,
            } = input
            else {
                panic!("a successful rebinding case");
            };
            assert_eq!(expected.unsubscribe_cleared, Some(true));
            assert_eq!(
                expected.calls.first().map(String::as_str) == Some("unsubscribe"),
                subscription
            );
            probe.removed.set(0);
            prepare_subscription(&world, &probe, subscription);
            let hidden = !world.mode.hide_thinking_block.get();
            world
                .mode
                .runtime_host
                .session()
                .settings_manager()
                .borrow_mut()
                .set_hide_thinking_block(hidden);
            let titles = titles(&world);
            world.mode.rebind_current_session().await.unwrap();
            assert_eq!(
                probe.seen.borrow().last().unwrap(),
                &json!({
                    "ui": true,
                    "removed": usize::from(subscription),
                    "hidden": hidden,
                    "subscribed": false,
                    "settingsFree": true,
                }),
                "the old subscription is gone and settings are applied before binding"
            );
            assert_eq!(
                probe.removed.get(),
                usize::from(subscription),
                "removed once"
            );
            assert!(
                world.mode.unsubscribe.borrow().is_some(),
                "a new subscription"
            );
            assert_eq!(titles.borrow().len(), 1, "the title is written last");
        }
    });
}

/// Arms the failure of `phase` for the next rebinding of `world`.
fn fail_rebind_phase(world: &World, probe: &Probe, phase: &str) {
    match phase {
        "applyRuntimeSettings" => {
            world
                .mode
                .runtime_host
                .session()
                .settings_manager()
                .borrow_mut()
                .set_show_hardware_cursor(false);
            world
                .terminal
                .set_hide_error(Some(std::io::ErrorKind::PermissionDenied));
        }
        "bindCurrentSessionExtensions" => {
            probe.fail.set(true);
            colourless(world);
        }
        "updateAvailableProviderCount" => super::models::refuse_refresh(world),
        "updateTerminalTitle" => world
            .terminal
            .set_title_error(Some(std::io::ErrorKind::BrokenPipe)),
        _ => unreachable!("{phase}"),
    }
}

#[test]
fn session_rebinding_stops_at_each_failed_phase() {
    const NAME: &str = "session_rebinding_stops_at_each_failed_phase";
    isolated(NAME, async |directory| {
        for (input, expected) in recorded::<Phases>(NAME) {
            let Input::Rebind {
                subscription,
                fail: Some(phase),
            } = input
            else {
                panic!("a failing rebinding case");
            };
            assert!(expected.error.is_some());
            if phase == "updateEditorBorderColor" {
                // The border's only failure is an unpublished theme, which the listing before
                // it has already reported; no later failure is reachable here.
                assert_eq!(expected.calls.last(), Some(&phase));
            } else {
                failed_rebind(directory, subscription, &phase, &expected.calls).await;
            }
        }
    });
}

/// Rebinds a started mode whose `phase` fails, with or without a previous `subscription`,
/// and checks that exactly the phases in `calls` ran, with no rollback.
async fn failed_rebind(directory: &Path, subscription: bool, phase: &str, calls: &[String]) {
    let probe = Rc::new(Probe::default());
    let world = Setup {
        extension: Some(probing(&probe)),
        configure: Box::new(|settings| settings.set_show_hardware_cursor(true)),
        ..Setup::default()
    }
    .build(directory);
    *probe.mode.borrow_mut() = Rc::downgrade(&world.mode);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    prepare_subscription(&world, &probe, subscription);
    probe.seen.borrow_mut().clear();
    fail_rebind_phase(&world, &probe, phase);
    let titles = titles(&world);
    let provider = &world.mode.footer_data_provider;
    provider.set_available_provider_count(-1.0);
    let error = world.mode.rebind_current_session().await.unwrap_err();
    let reached = |name: &str| calls.iter().any(|call| call == name);
    assert_eq!(probe.removed.get(), usize::from(subscription));
    assert_eq!(
        probe.seen.borrow().len(),
        usize::from(reached("bindCurrentSessionExtensions")),
        "{phase}: {error}"
    );
    assert_eq!(
        world.mode.unsubscribe.borrow().is_some(),
        reached("subscribeToAgent"),
        "{phase}"
    );
    assert_eq!(
        provider.get_available_provider_count() >= 0.0,
        reached("updateEditorBorderColor"),
        "{phase}: the provider count is refreshed only before a later failure"
    );
    assert!(titles.borrow().is_empty(), "{phase}: no title is written");
    let kind = error
        .downcast_ref::<std::io::Error>()
        .map(std::io::Error::kind);
    let expected = match phase {
        "applyRuntimeSettings" => Some(std::io::ErrorKind::PermissionDenied),
        "updateTerminalTitle" => Some(std::io::ErrorKind::BrokenPipe),
        _ => None,
    };
    assert_eq!(kind, expected, "{phase}: {error}");
}

/// A discovery gate: the next discovery waits for its sender.
type Gate = Rc<RefCell<Option<tokio::sync::oneshot::Receiver<()>>>>;

/// An extension that asks through the started UI on session start, records each phase
/// in `log`, waits at discovery while `gate` holds a receiver, then fails discovery when
/// `refuse` is set.
fn asking(
    log: &Rc<RefCell<Vec<String>>>,
    gate: &Gate,
    refuse: &Rc<Cell<bool>>,
) -> ExtensionFactory<Local> {
    let (log, gate, refuse) = (Rc::clone(log), Rc::clone(gate), Rc::clone(refuse));
    Rc::new(move |api| {
        let started = Rc::clone(&log);
        let start = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, context| {
                let present = context.has_ui().unwrap();
                Box::pin(ask(Rc::clone(&started), context.ui().unwrap(), present))
            }),
        );
        let (discovered, gate, refuse) = (Rc::clone(&log), Rc::clone(&gate), Rc::clone(&refuse));
        let discover = api.on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
            "resources_discover".into(),
            Rc::new(move |_, _| {
                let gate = gate.borrow_mut().take();
                Box::pin(discover(Rc::clone(&discovered), gate, Rc::clone(&refuse)))
            }),
        );
        Box::pin(std::future::ready(start.and(discover).map(drop)))
    })
}

/// Records whether the started UI is `present`, then asks through `ui`.
async fn ask(
    log: Rc<RefCell<Vec<String>>>,
    ui: maestro_app::ExtensionUIContext,
    present: bool,
) -> Result<Option<()>, maestro_app::EventError> {
    log.borrow_mut().push(format!("bind ui={present}"));
    let chosen = ui
        .select("Choose".to_owned(), vec!["answer".to_owned()], None)
        .await?;
    log.borrow_mut().push(format!("chose {chosen:?}"));
    Ok(None)
}

/// Records discovery, waits for `gate` when one is armed, then fails when `refuse` is set.
async fn discover(
    log: Rc<RefCell<Vec<String>>>,
    gate: Option<tokio::sync::oneshot::Receiver<()>>,
    refuse: Rc<Cell<bool>>,
) -> Result<Option<ResourcesDiscoverResult>, maestro_app::EventError> {
    log.borrow_mut().push("discover".to_owned());
    if let Some(gate) = gate {
        gate.await.unwrap();
    }
    log.borrow_mut().push("discovered".to_owned());
    if refuse.get() {
        return Err(Box::new(DiagnosticErrorInfo::new(None, "refused")));
    }
    Ok(None)
}

/// Registers `shortcut` on the first loaded extension under that extension's path.
fn register_shortcut(owners: &LoadExtensionsResult<Local>, mut shortcut: ExtensionShortcut<Local>) {
    let owner = &owners.extensions[0];
    shortcut.extension_path = owner.read().unwrap().path.clone();
    owner
        .write()
        .unwrap()
        .shortcuts
        .insert(shortcut.shortcut.clone(), Rc::new(RwLock::new(shortcut)));
}

/// A `ctrl+q` shortcut that runs `handler`.
fn ctrl_q(
    handler: impl Fn(maestro_app::ExtensionContext) -> Result<(), maestro_app::EventError> + 'static,
) -> ExtensionShortcut<Local> {
    let handler = Rc::new(handler);
    ExtensionShortcut {
        shortcut: "ctrl+q".to_owned(),
        description: None,
        handler: Rc::new(move |context| {
            let handler = Rc::clone(&handler);
            Box::pin(async move { handler(context) })
        }),
        extension_path: String::new(),
    }
}

/// Yields until `log` holds `entry`.
async fn until_logged(log: &Rc<RefCell<Vec<String>>>, entry: &str) {
    for _ in 0..10_000 {
        if log.borrow().iter().any(|seen| seen == entry) {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("{entry} in {:?}", log.borrow());
}

/// The transcript's plain rows.
fn transcript(mode: &InteractiveMode) -> Vec<String> {
    mode.chat_container
        .render(80)
        .iter()
        .map(|row| strip_ansi(row).unwrap().trim_end().to_owned())
        .collect()
}

#[test]
fn session_binding_waits_for_extensions_before_completion_shortcuts_resources_and_notices() {
    const NAME: &str =
        "session_binding_waits_for_extensions_before_completion_shortcuts_resources_and_notices";
    isolated(NAME, async |directory| {
        for (input, expected) in recorded::<Phases>(NAME) {
            let Input::Binding { fail } = input else {
                panic!("a binding case");
            };
            assert_eq!(expected.ui_forwarded, Some(true));
            binding_case(directory, fail.as_deref(), &expected).await;
        }
    });
}

/// A mode whose initialization waits at discovery, after its session start asked through
/// the started UI.
struct Paused {
    /// The mode and its effects.
    world: World,
    /// The extension's phase records.
    log: Rc<RefCell<Vec<String>>>,
    /// Lets discovery continue.
    release: tokio::sync::oneshot::Sender<()>,
    /// Makes the waiting discovery fail.
    refuse: Rc<Cell<bool>>,
    /// The initialization, settling with its failure.
    started: tokio::task::JoinHandle<Option<Box<dyn std::error::Error>>>,
}

/// Starts a mode, quiet as selected, with one context file and one shortcut, answers its
/// session start's question and checks that nothing after binding has run at discovery.
async fn paused(directory: &Path, quiet: bool) -> Paused {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (release, receiver) = tokio::sync::oneshot::channel();
    let gate: Gate = Rc::new(RefCell::new(Some(receiver)));
    let refuse = Rc::new(Cell::new(false));
    let world = Setup {
        extension: Some(asking(&log, &gate, &refuse)),
        configure: Box::new(move |settings| settings.set_quiet_startup(quiet)),
        context: vec![maestro_app::ContextFile {
            path: directory.join("AGENTS.md").to_str().unwrap().to_owned(),
            content: "context".to_owned(),
        }],
        ..Setup::default()
    }
    .build(directory);
    register_shortcut(
        &world
            .mode
            .runtime_host
            .session()
            .resource_loader()
            .get_extensions(),
        ctrl_q(|_| Ok(())),
    );
    let mode = Rc::clone(&world.mode);
    let started = tokio::task::spawn_local(async move { mode.init().await.err() });
    until_logged(&log, "bind ui=true").await;
    world.runtime.settle().unwrap();
    world.terminal.send_input("\r");
    until_logged(&log, "discover").await;
    world.loader.calls.borrow_mut().clear();
    assert!(world.mode.editor.on_extension_shortcut().is_none());
    assert!(
        world.mode.unsubscribe.borrow().is_none(),
        "no subscription yet"
    );
    let rows = transcript(&world.mode);
    assert!(rows.iter().all(String::is_empty), "nothing listed yet");
    Paused {
        world,
        log,
        release,
        refuse,
        started,
    }
}

/// Asserts that a binding stopped at `fail` returned the colour failure: the bound
/// listener's failure stops discovery as the session's failure, and the later phases fail
/// with the colour failure itself.
fn assert_binding_error(fail: Option<&str>, error: Option<&(dyn std::error::Error + 'static)>) {
    let Some(error) = error else {
        return;
    };
    let typed = if fail == Some("bind") {
        error.downcast_ref::<DiagnosticErrorInfo>().is_some()
    } else {
        error.downcast_ref::<maestro_theme::ThemeError>().is_some()
    };
    assert!(typed, "{fail:?}: {error}");
    assert!(
        error.to_string().starts_with("Unknown theme color"),
        "{error}"
    );
}

/// Starts a mode whose binding fails at `fail` and checks the phases `expected` reaches.
async fn binding_case(directory: &Path, fail: Option<&str>, expected: &Phases) {
    let paused = paused(directory, fail == Some("notices")).await;
    let world = &paused.world;
    *world.mode.changelog_markdown.borrow_mut() = Some("## [9.9.9]\n- change".to_owned());
    if fail.is_some() {
        paused.refuse.set(fail == Some("bind"));
        colourless(world);
    }
    paused.release.send(()).unwrap();
    let error = paused.started.await.unwrap();
    let asked = [
        "bind ui=true",
        "chose Some(\"answer\")",
        "discover",
        "discovered",
    ];
    assert_eq!(*paused.log.borrow(), asked);
    assert_eq!(
        error.is_some(),
        expected.error.is_some(),
        "{fail:?}: {error:?}"
    );
    assert_binding_error(fail, error.as_deref());
    let reached = |name: &str| expected.calls.iter().any(|call| call == name);
    let reads = world.loader.calls.borrow().clone();
    assert_eq!(
        reads.first() == Some(&"themes"),
        reached("themes"),
        "{fail:?}: {reads:?}"
    );
    assert_eq!(
        world.mode.editor.on_extension_shortcut().is_some(),
        reached("shortcuts"),
        "{fail:?}"
    );
    assert_eq!(world.mode.startup_notices_shown.get(), reached("notices"));
    let rows = transcript(&world.mode);
    let listed = rows.iter().position(|row| row.contains("[Context]"));
    let noticed = rows.iter().position(|row| row.contains("What's New"));
    assert_eq!(listed.is_some(), fail.is_none(), "{fail:?}: {rows:?}");
    assert_eq!(noticed.is_some(), fail.is_none(), "{fail:?}: {rows:?}");
    assert!(listed <= noticed, "the listing comes before the notices");
    assert_eq!(world.mode.unsubscribe.borrow().is_some(), fail.is_none());
}

/// Requests shutdown through a context created while `created` streams, while `requested`
/// streams, after recording `before`, and checks whether the handler itself started it.
async fn shutdown_case(
    directory: &Path,
    (created, requested, before): (bool, bool, bool),
    expected: &Requested,
) {
    let effects = Rc::new(super::lifecycle::Effects::default());
    let world = Setup {
        lifecycle: effects.clone(),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world.mode.shutdown_requested.set(before);
    let runner = || world.mode.runtime_host.session().extension_runner();
    let started = Rc::new(Cell::new(0));
    let request = |context: &maestro_app::ExtensionContext| {
        context.shutdown().unwrap();
        started.set(world.runtime.futures());
    };
    match (created, requested) {
        (false, false) => request(&runner().create_context()),
        (true, true) => {
            super::streaming(&world, async |_, _| request(&runner().create_context())).await;
        }
        (false, true) => {
            let context = runner().create_context();
            super::streaming(&world, async |_, _| request(&context)).await;
        }
        (true, false) => {
            let context = super::streaming(&world, async |_, _| runner().create_context()).await;
            request(&context);
        }
    }
    assert_eq!(world.mode.shutdown_requested.get(), expected.requested);
    assert_eq!(
        started.get(),
        usize::from(expected.calls == ["shutdown"]),
        "the handler starts shutdown only while idle"
    );
    request(&runner().create_context());
    assert!(world.runtime.drain_futures().is_empty());
    assert_eq!(
        exits(&effects),
        1,
        "cleanup and disposal run once: {:?}",
        effects.events.borrow()
    );
    assert!(!world.mode.is_initialized.get());
}

/// Starts a mode whose second extension, at `/extensions/bound`, fails its session start
/// with `stack`, and returns the transcript rows that failure added.
async fn bound_failure(directory: &Path, stack: Option<String>) -> Vec<String> {
    let failing: ExtensionFactory<Local> = Rc::new(move |api| {
        let stack = stack.clone();
        let registered = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, _| {
                let mut error = DiagnosticErrorInfo::new(None, "binding failure");
                error.stack.clone_from(&stack);
                Box::pin(std::future::ready(Err(
                    Box::new(error) as maestro_app::EventError
                )))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    });
    let world = Setup {
        extension: Some(idle()),
        further: vec![("/extensions/bound".to_owned(), failing)],
        configure: Box::new(|settings| settings.set_quiet_startup(true)),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    transcript(&world.mode)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

#[test]
fn session_binding_forwards_native_shutdown_and_extension_errors() {
    const NAME: &str = "session_binding_forwards_native_shutdown_and_extension_errors";
    isolated(NAME, async |directory| {
        for (input, expected) in cases::<Input, Value>(FIXTURES, NAME) {
            match input {
                Input::BoundShutdown { streaming, before } => {
                    let expected = serde_json::from_value(expected).unwrap();
                    shutdown_case(directory, (streaming, streaming, before), &expected).await;
                }
                Input::BoundShutdownTransition { before, after } => {
                    let expected = serde_json::from_value(expected).unwrap();
                    shutdown_case(directory, (before, after, false), &expected).await;
                }
                Input::BoundError { stack } => {
                    bound_error_case(directory, stack, serde_json::from_value(expected).unwrap())
                        .await;
                }
                _ => panic!("a shutdown or failure case"),
            }
        }
    });
}

/// Checks that a session-start failure with `stack` reaches the transcript once, as the
/// one failure `expected` records.
async fn bound_error_case(directory: &Path, stack: Option<String>, expected: Forwarded) {
    let [call] = &expected.calls[..] else {
        panic!("one failure");
    };
    assert_eq!(call.stack, stack);
    let heading = format!(
        " Extension \"{}\" error: {}",
        call.extension_path, call.error
    );
    let frames = stack
        .iter()
        .flat_map(|stack| stack.split('\n').skip(1))
        .map(|frame| {
            format!(
                "   {}",
                frame.trim_matches(maestro_app::is_whitespace_scalar)
            )
        });
    let rows: Vec<String> = std::iter::once(heading).chain(frames).collect();
    assert_eq!(bound_failure(directory, stack.clone()).await, rows);
}

/// How many times the process exit `effects` record ran with status zero.
fn exits(effects: &super::lifecycle::Effects) -> usize {
    effects
        .events
        .borrow()
        .iter()
        .filter(|event| *event == "exit:0")
        .count()
}

/// Presses `ctrl+q` and finishes the work it starts, settling frames when `idle`; a
/// streaming response keeps its working animation scheduled.
fn press_ctrl_q(world: &World, idle: bool) {
    world.mode.editor.handle_input("\x11");
    if idle {
        world.runtime.settle().unwrap();
    }
    assert!(world.runtime.drain_futures().is_empty());
}

/// Requests shutdown through the installed shortcut, while `streaming` or idle, and
/// checks that the installed handler ends the mode once, after the response when streaming.
async fn shortcut_shutdown(directory: &Path, streaming: bool) {
    let effects = Rc::new(super::lifecycle::Effects::default());
    let world = Setup {
        lifecycle: effects.clone(),
        extension: Some(idle()),
        ..Setup::default()
    }
    .build(directory);
    register_shortcut(
        &world
            .mode
            .runtime_host
            .session()
            .resource_loader()
            .get_extensions(),
        ctrl_q(|context| context.shutdown()),
    );
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    if streaming {
        super::streaming(&world, async |world, _| {
            press_ctrl_q(world, false);
            assert!(world.mode.shutdown_requested.get());
            assert_eq!(
                exits(&effects),
                0,
                "a streaming request waits for the agent"
            );
        })
        .await;
        assert!(world.runtime.drain_futures().is_empty());
    } else {
        press_ctrl_q(&world, true);
    }
    assert!(world.mode.shutdown_requested.get());
    assert_eq!(exits(&effects), 1, "{:?}", effects.events.borrow());
    assert!(!world.mode.is_initialized.get(), "the terminal is stopped");
}

#[test]
fn idle_shortcut_shutdown_reaches_the_installed_shutdown_handler() {
    isolated(
        "idle_shortcut_shutdown_reaches_the_installed_shutdown_handler",
        async |directory| {
            for streaming in [false, true] {
                shortcut_shutdown(directory, streaming).await;
            }
        },
    );
}

/// Reports `stack` through the runner's attributed-failure route of `world`.
fn report(world: &World, stack: Option<String>) {
    world
        .mode
        .runtime_host
        .session()
        .extension_runner()
        .emit_error(&ExtensionError {
            extension_path: "/extensions/a".to_owned(),
            event: "session_start".to_owned(),
            error: "failure".to_owned(),
            stack,
        })
        .unwrap();
}

#[test]
fn extension_errors_keep_the_heading_and_trimmed_stack_tail() {
    const NAME: &str = "extension_errors_keep_the_heading_and_trimmed_stack_tail";
    isolated(NAME, async |directory| {
        let world = started(directory).await;
        let theme = world.themes.theme().get().unwrap();
        for (input, expected) in recorded::<Shown>(NAME) {
            let Input::ExtensionError { stack } = input else {
                panic!("a failure case");
            };
            world.mode.chat_container.clear();
            world.runtime.settle().unwrap();
            report(&world, stack);
            assert_eq!(world.runtime.pending() > 0, expected.render_requested);
            let children = world.mode.chat_container.children().borrow().len();
            assert_eq!(children, expected.children);
            let rows = world.mode.chat_container.render(80);
            assert_eq!(rows, super::painted(&theme, &expected.lines));
            let texts: Vec<String> = expected
                .texts
                .iter()
                .flat_map(|text| text.split('\n'))
                .map(|row| format!(" {}", roles(&theme, row)))
                .collect();
            assert_eq!(rows.len(), texts.len());
            for (row, text) in rows.iter().zip(&texts) {
                assert!(row.starts_with(text), "{row:?} starts with {text:?}");
            }
        }
    });
}

#[test]
fn extension_error_stacks_keep_every_frame_at_scale() {
    isolated(
        "extension_error_stacks_keep_every_frame_at_scale",
        async |directory| {
            let world = started(directory).await;
            for frames in [32, 64, 128] {
                world.mode.chat_container.clear();
                let stack = std::iter::once("Error: failure".to_owned())
                    .chain((0..frames).map(|frame| format!("    at frame{frame} (a.js:{frame}:1)")))
                    .collect::<Vec<_>>()
                    .join("\n");
                report(&world, Some(stack));
                let expected: Vec<String> =
                    std::iter::once(" Extension \"/extensions/a\" error: failure".to_owned())
                        .chain(
                            (0..frames).map(|frame| format!("   at frame{frame} (a.js:{frame}:1)")),
                        )
                        .collect();
                assert_eq!(transcript(&world.mode), expected, "{frames} frames");
            }
        },
    );
}

/// What a shortcut's context reads, with the catalog compared to `registry`.
fn capabilities(
    context: &maestro_app::ExtensionContext,
    registry: &std::sync::Arc<maestro_app::ModelRegistry>,
) -> Result<Value, maestro_app::EventError> {
    let manager = context.session_manager()?;
    let cwd = manager.borrow().get_cwd().to_owned();
    Ok(json!({
        "ui": context.has_ui()?,
        "cwd": context.cwd()?,
        "manager": cwd,
        "registry": std::sync::Arc::ptr_eq(&context.model_registry()?, registry),
        "model": context.model()?.is_some(),
        "idle": context.is_idle()?,
        "signal": context.signal()?.is_some(),
        "pending": context.has_pending_messages()?,
        "usage": context.get_context_usage()?.is_some(),
        "prompt": !context.get_system_prompt()?.is_empty(),
    }))
}

/// A `ctrl+q` shortcut that writes the editor through its UI, records its context's
/// reads in `log`, aborts, compacts with its failure recorded in `log`, then shuts down.
fn capability_shortcut(
    log: &Rc<RefCell<Vec<Value>>>,
    registry: std::sync::Arc<maestro_app::ModelRegistry>,
) -> ExtensionShortcut<Local> {
    let log = Rc::clone(log);
    ctrl_q(move |context| {
        context.ui()?.set_editor_text("from shortcut".to_owned())?;
        log.borrow_mut().push(capabilities(&context, &registry)?);
        context.abort()?;
        let failed = Rc::clone(&log);
        context.compact(Some(maestro_app::CompactOptions {
            custom_instructions: None,
            on_complete: None,
            on_error: Some(Rc::new(move |error| {
                failed
                    .borrow_mut()
                    .push(json!({"compact": error.to_string()}));
                Ok(())
            })),
        }))?;
        context.shutdown()
    })
}

#[test]
fn shortcut_context_exposes_current_runner_capabilities() {
    isolated(
        "shortcut_context_exposes_current_runner_capabilities",
        async |directory| {
            let effects = Rc::new(super::lifecycle::Effects::default());
            let world = Setup {
                lifecycle: effects.clone(),
                extension: Some(idle()),
                ..Setup::default()
            }
            .build(directory);
            let log = Rc::new(RefCell::new(Vec::new()));
            let session = world.mode.runtime_host.session();
            let registry = std::sync::Arc::clone(session.model_registry());
            register_shortcut(
                &session.resource_loader().get_extensions(),
                capability_shortcut(&log, registry),
            );
            world
                .host
                .services()
                .auth_storage
                .set_runtime_api_key("unknown", "key");
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            world.mode.editor.handle_input("\x11");
            world.runtime.settle().unwrap();
            for _ in 0..10_000 {
                if log.borrow().len() == 2 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert!(world.runtime.drain_futures().is_empty());
            let cwd = session.session_manager().borrow().get_cwd().to_owned();
            let usage = session.get_context_usage().unwrap().is_some();
            assert_eq!(
                *log.borrow(),
                [
                    json!({
                        "ui": true,
                        "cwd": cwd,
                        "manager": cwd,
                        "registry": true,
                        "model": true,
                        "idle": true,
                        "signal": false,
                        "pending": false,
                        "usage": usage,
                        "prompt": true,
                    }),
                    json!({"compact": "Nothing to compact (session too small)"}),
                ]
            );
            assert_eq!(world.mode.editor.get_text(), "from shortcut");
            assert!(world.mode.shutdown_requested.get());
            assert_eq!(exits(&effects), 1, "{:?}", effects.events.borrow());
        },
    );
}

#[test]
fn native_binding_owned_context_keeps_the_mode_and_then_restores_no_ui() {
    isolated(
        "native_binding_owned_context_keeps_the_mode_and_then_restores_no_ui",
        async |directory| {
            let world = started(directory).await;
            let session = world.mode.runtime_host.session();
            let runner = session.extension_runner();
            let context = runner.create_context();
            assert!(context.has_ui().unwrap());
            let ui = context.ui().unwrap();
            let weak = Rc::downgrade(&world.mode);
            drop(world.mode);
            assert!(
                weak.upgrade().is_some(),
                "an obtained UI keeps the mode alive"
            );
            ui.set_editor_text("retained host".to_owned()).unwrap();
            assert_eq!(ui.get_editor_text().unwrap(), "retained host");
            drop(ui);
            assert!(
                weak.upgrade().is_none(),
                "session and runner do not own the mode"
            );
            assert!(!context.has_ui().unwrap());
            assert!(!runner.has_ui());
            assert_eq!(context.ui().unwrap().get_editor_text().unwrap(), "");
        },
    );
}

/// A `ctrl+q` shortcut that records which runner its context reads.
fn reading_shortcut(log: &Rc<RefCell<Vec<Value>>>) -> ExtensionShortcut<Local> {
    let log = Rc::clone(log);
    ctrl_q(move |context| {
        let model = context
            .model()?
            .map(|model| model.read().unwrap().id.clone());
        log.borrow_mut().push(json!({
            "ui": context.has_ui()?,
            "cwd": context.cwd()?,
            "model": model,
            "idle": context.is_idle()?,
            "pending": context.has_pending_messages()?,
            "prompt": !context.get_system_prompt()?.is_empty(),
        }));
        Ok(())
    })
}

/// A replacement factory over fresh services; each replacement lists one context file,
/// selects the model `replacement-<n>` and loads `extension` with `reading_shortcut`.
pub(super) fn renaming_replacements(
    directory: &Path,
    log: &Rc<RefCell<Vec<Value>>>,
    extension: &ExtensionFactory<Local>,
) -> maestro_app::CreateAgentSessionRuntimeFactory {
    let created = Rc::new(Cell::new(0));
    let context = directory.join("AGENTS.md").to_str().unwrap().to_owned();
    let log = Rc::clone(log);
    let extension = Rc::clone(extension);
    Rc::new(move |options| {
        created.set(created.get() + 1);
        let extensions = super::loaded(&options.cwd, Rc::clone(&extension), Vec::new());
        register_shortcut(&extensions, reading_shortcut(&log));
        let resources = super::Resources {
            context: vec![maestro_app::ContextFile {
                path: context.clone(),
                content: "context".to_owned(),
            }],
            skills: Vec::new(),
            prompts: Vec::new(),
            themes: Vec::new(),
            extensions: Vec::new(),
            errors: Vec::new(),
            commands: Vec::new(),
            diagnostics: super::LoadedDiagnostics::default(),
        };
        let replacement = super::session::session(&options.cwd, None, |inner| {
            Rc::new(super::ForwardingLoader::new(
                inner,
                resources,
                Some(extensions),
            ))
        });
        let state = replacement.session.state();
        let mut model = state.read().unwrap().model.read().unwrap().clone();
        model.id = format!("replacement-{}", created.get());
        state.write().unwrap().model = std::sync::Arc::new(std::sync::RwLock::new(model));
        std::mem::swap(
            &mut *replacement.session.session_manager().borrow_mut(),
            &mut *options.session_manager.borrow_mut(),
        );
        Box::pin(async { Ok(replacement) })
    })
}

/// What the replacement continuation observes of the mode and its new context.
fn continued(
    mode: &Weak<InteractiveMode>,
    seen: &Rc<RefCell<Vec<Value>>>,
    titles: &Rc<RefCell<Vec<String>>>,
) -> maestro_app::NewSessionCommandOptions {
    let mode = Weak::clone(mode);
    let seen = Rc::clone(seen);
    let titles = Rc::clone(titles);
    maestro_app::NewSessionCommandOptions {
        with_session: Some(Rc::new(move |context| {
            let mode = mode.upgrade().unwrap();
            let seen = Rc::clone(&seen);
            let written = titles.borrow().len();
            Box::pin(async move {
                let ui = context.has_ui()?;
                context.ui()?.set_editor_text("continued".to_owned())?;
                let listed = transcript(&mode)
                    .iter()
                    .filter(|row| row.contains("[Context]"))
                    .count();
                seen.borrow_mut().push(json!({
                    "ui": ui,
                    "editor": mode.editor.get_text(),
                    "installed": installed(&mode),
                    "titled": written,
                    "listed": listed,
                }));
                Ok(())
            })
        })),
        ..maestro_app::NewSessionCommandOptions::default()
    }
}

/// Whether `mode`'s footer shows the current session's model, the mode is subscribed to
/// session events and an extension shortcut is installed.
pub(super) fn installed(mode: &InteractiveMode) -> Value {
    let model = mode
        .runtime_host
        .session()
        .model()
        .read()
        .unwrap()
        .id
        .clone();
    json!({
        "footer": mode.footer.render(200).iter().any(|row| row.contains(&model)),
        "subscribed": mode.unsubscribe.borrow().is_some(),
        "shortcut": mode.editor.on_extension_shortcut().is_some(),
    })
}

/// What the next `ctrl+q` press reads.
fn next_shortcut_reads(world: &World, log: &RefCell<Vec<Value>>) -> Vec<Value> {
    log.borrow_mut().clear();
    press_ctrl_q(world, true);
    log.take()
}

/// The session a replacement retires, captured before the replacement runs.
pub(super) struct Retiring {
    /// The retired session's model.
    initial: String,
    /// The working directory the replacement keeps.
    cwd: String,
    /// The retired session.
    session: Rc<maestro_app::AgentSession>,
    /// A context taken from the retired session's runner.
    context: maestro_app::ExtensionContext,
    /// Each terminal title written since the capture.
    pub(super) titles: Rc<RefCell<Vec<String>>>,
}

impl Retiring {
    /// Captures `world`'s current session and starts recording terminal titles.
    pub(super) fn capture(world: &World) -> Self {
        let session = world.host.session();
        let initial = session.model().read().unwrap().id.clone();
        let cwd = session.session_manager().borrow().get_cwd().to_owned();
        Self {
            initial,
            cwd,
            context: session.extension_runner().create_context(),
            session,
            titles: titles(world),
        }
    }

    /// Checks that the `generation`th replacement retired the captured session, its context
    /// and its events, and that the next shortcut reads the replacement runner.
    pub(super) fn assert_replaced(
        self,
        world: &World,
        generation: usize,
        log: &RefCell<Vec<Value>>,
    ) {
        let Self {
            initial,
            cwd,
            session,
            context,
            titles,
        } = self;
        assert!(!Rc::ptr_eq(&session, &world.host.session()));
        assert!(context.is_idle().is_err(), "the retired context is stale");
        titles.borrow_mut().clear();
        world.runtime.settle().unwrap();
        session.set_session_name("retired").unwrap();
        assert!(titles.borrow().is_empty(), "retired events do not repaint");
        let retired = Rc::downgrade(&session);
        drop((session, context));
        assert!(
            retired.upgrade().is_none(),
            "the mode releases the retired session"
        );
        let model = format!("replacement-{generation}");
        assert_ne!(model, initial, "the replacement selects another model");
        assert_eq!(
            next_shortcut_reads(world, log),
            [json!({
                "ui": true,
                "cwd": cwd,
                "model": model,
                "idle": true,
                "pending": false,
                "prompt": true,
            })],
            "the next shortcut reads the replacement runner"
        );
    }
}

/// Replaces the session for the `generation`th time through the runtime, then checks the
/// continuation, the retired session and context, and what the next shortcut reads.
async fn replace_once(world: &World, generation: usize, log: &Rc<RefCell<Vec<Value>>>) {
    let retiring = Retiring::capture(world);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let options = continued(&Rc::downgrade(&world.mode), &seen, &retiring.titles);
    let result = world.host.new_session(options).await.unwrap();
    assert!(!result.cancelled);
    assert_eq!(
        *seen.borrow(),
        [json!({
            "ui": true,
            "editor": "continued",
            "installed": {"footer": true, "subscribed": true, "shortcut": true},
            "titled": 2,
            "listed": generation + 1,
        })],
        "the continuation sees the completed binding"
    );
    retiring.assert_replaced(world, generation, log);
}

#[test]
fn real_session_replacement_rebinds_shortcuts_before_continuation() {
    isolated(
        "real_session_replacement_rebinds_shortcuts_before_continuation",
        async |directory| {
            let log = Rc::new(RefCell::new(Vec::new()));
            let world = Setup {
                extension: Some(idle()),
                create: Some(renaming_replacements(directory, &log, &idle())),
                context: vec![maestro_app::ContextFile {
                    path: directory.join("AGENTS.md").to_str().unwrap().to_owned(),
                    content: "context".to_owned(),
                }],
                ..Setup::default()
            }
            .build(directory);
            register_shortcut(
                &world
                    .mode
                    .runtime_host
                    .session()
                    .resource_loader()
                    .get_extensions(),
                reading_shortcut(&log),
            );
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            for generation in 1..=2 {
                replace_once(&world, generation, &log).await;
            }
        },
    );
}
