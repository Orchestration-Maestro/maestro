//! The extension UI host over the retained mode: theme, editor, title, expansion and
//! typed errors through the installed port.
mod dialogs;
mod input;
mod notifications;

use super::transcript::tool_params;
use super::{Setup, World, dark, isolated, light, roles, started};
use crate::InteractiveMode;
use maestro_app::presentation_data::footer_data_provider::{
    FooterFileStats, FooterOperations, FsWatcher, WatchOperations, WatchTimer,
};
use maestro_app::{
    DiagnosticErrorInfo, ExtensionUIContext, SettingsManager, SettingsScope, SettingsStorage,
    SettingsStorageError, SettingsUpdate, TextContent, ThemeSelection, UserBlock,
};
use maestro_theme::{
    ColorMode, LiveTheme, Theme, ThemeColor, ThemeDirectories, ThemeInfo, ThemeOptions,
};
use maestro_tui::Component;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    io,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

/// Every recorded host case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_extension_ui.json");

/// The text the recorded paste rows deliver: eleven lines, one more than the editor keeps
/// inline, so its raw text holds a marker while the expanded text holds the paste.
const PASTED: &str = "paste\n界\nrow 2\nrow 3\nrow 4\nrow 5\nrow 6\nrow 7\nrow 8\nrow 9\nrow 10";

/// A started mode whose registry reads the shipped theme documents of `directory`.
async fn registered(directory: &Path) -> World {
    let world = Setup {
        directories: Some(theme_dirs(directory)),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The error colour the published theme paints with.
fn error_escape(theme: &Theme) -> String {
    theme
        .get_fg_ansi(&ThemeColor::Error)
        .unwrap_or_default()
        .to_owned()
}

/// Settles the runtime and checks that the frame it paints uses the border and dim colours
/// of `selected` and none of `replaced`'s.
fn assert_repainted(world: &World, selected: &Theme, replaced: &Theme) {
    world.terminal.clear_writes();
    world.runtime.settle().unwrap();
    let frame = world.terminal.writes().concat();
    for role in [ThemeColor::ThinkingOff, ThemeColor::Dim] {
        let new = selected.get_fg_ansi(&role).unwrap();
        let old = replaced.get_fg_ansi(&role).unwrap();
        assert_ne!(new, old, "{role:?} tells the two themes apart");
        assert!(frame.contains(new), "the frame paints {role:?} as selected");
        assert!(!frame.contains(old), "the frame keeps no replaced {role:?}");
    }
}

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    super::cases(FIXTURES, test)
}

/// The current runner's UI context after the mode's native binding.
pub(super) fn port(mode: &Rc<InteractiveMode>) -> ExtensionUIContext {
    let context = mode
        .runtime_host
        .session()
        .extension_runner()
        .create_context();
    assert!(context.has_ui().unwrap(), "startup installed the native UI");
    context.ui().unwrap()
}

/// Copies the shipped theme documents into `root` and names the registry directories.
fn theme_dirs(root: &Path) -> ThemeDirectories {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-theme/assets/theme");
    let themes_dir = root.join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::create_dir_all(root.join("custom")).unwrap();
    for name in ["dark.json", "light.json"] {
        std::fs::copy(shipped.join(name), themes_dir.join(name)).unwrap();
    }
    ThemeDirectories {
        themes_dir: themes_dir.to_string_lossy().into_owned(),
        custom_themes_dir: root.join("custom").to_string_lossy().into_owned(),
    }
}

/// Adds one custom theme document to the registry directories of `root`.
pub(super) fn with_custom_theme(root: &Path) -> ThemeDirectories {
    let directories = theme_dirs(root);
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-theme/assets/theme");
    let document = std::fs::read_to_string(shipped.join("light.json"))
        .unwrap()
        .replace("\"light\"", "\"custom\"");
    std::fs::write(
        Path::new(&directories.custom_themes_dir).join("custom.json"),
        document,
    )
    .unwrap();
    directories
}

/// Settings storage recording every written document.
#[derive(Default)]
struct RecordingStorage {
    /// Stored text per scope.
    texts: Mutex<[Option<String>; 2]>,
    /// Every written document, in order.
    writes: Mutex<Vec<String>>,
    /// Announces each written document to the test.
    written: Mutex<Option<Sender<String>>>,
}

impl RecordingStorage {
    /// Every document written so far, in order.
    fn writes(&self) -> Vec<String> {
        self.writes
            .lock()
            .map(|writes| writes.clone())
            .unwrap_or_default()
    }
}

impl SettingsStorage for RecordingStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let current = self.texts.lock().map_err(|_| failure())?[scope as usize].clone();
        let next = update(current.as_deref())?;
        if let Some(text) = next {
            self.texts.lock().map_err(|_| failure())?[scope as usize] = Some(text.clone());
            self.writes
                .lock()
                .map_err(|_| failure())?
                .push(text.clone());
            let sender = self
                .written
                .lock()
                .map_err(|_| failure())?
                .as_ref()
                .map(Sender::clone);
            if let Some(sender) = sender {
                drop(sender.send(text));
            }
        }
        Ok(())
    }
}

/// The failure reported by a poisoned recording lock.
fn failure() -> SettingsStorageError {
    Box::new(io::Error::other("poisoned recording storage"))
}

/// A recording storage seeded with `theme` and the receiver of its write announcements.
fn recording_storage(theme: &str) -> (Arc<RecordingStorage>, Receiver<String>) {
    let (sender, receiver) = channel();
    let storage = Arc::new(RecordingStorage {
        texts: Mutex::new([Some(format!("{{\n  \"theme\": \"{theme}\"\n}}")), None]),
        written: Mutex::new(Some(sender)),
        ..RecordingStorage::default()
    });
    (storage, receiver)
}

/// Replaces the mode's settings manager with one over `storage`.
fn record_settings(world: &World, storage: &Arc<RecordingStorage>) {
    let owned: Arc<RecordingStorage> = Arc::clone(storage);
    let handle: Arc<dyn SettingsStorage> = owned;
    let manager = SettingsManager::from_storage(handle);
    *world.host.session().settings_manager().borrow_mut() = manager;
}

/// Drains the ordered queue with one further edit and returns the theme names the
/// selection saved before it.
fn saved_themes(
    world: &World,
    storage: &RecordingStorage,
    receiver: &Receiver<String>,
) -> Vec<String> {
    world
        .host
        .session()
        .settings_manager()
        .borrow_mut()
        .set_quiet_startup(true);
    loop {
        let text = receiver
            .recv()
            .expect("the queue worker writes the barrier edit");
        let document: serde_json::Value = serde_json::from_str(&text).expect("a document");
        if document.get("quietStartup").is_some() {
            break;
        }
    }
    let mut themes = Vec::new();
    for text in storage.writes() {
        let document: serde_json::Value = serde_json::from_str(&text).expect("a document");
        if document.get("quietStartup").is_some() {
            break;
        }
        if let Some(theme) = document.get("theme").and_then(serde_json::Value::as_str) {
            themes.push(theme.to_owned());
        }
    }
    themes
}

/// Watch effects recording each registration and whether it was closed.
#[derive(Default)]
pub(super) struct RecordingWatcher {
    /// Each registration's watched directory and its closed flag.
    watches: RefCell<Vec<(String, Rc<Cell<bool>>)>>,
}

impl RecordingWatcher {
    /// The watched directories, in registration order.
    pub(super) fn paths(&self) -> Vec<String> {
        self.watches
            .borrow()
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Whether the registration at `index` was closed.
    pub(super) fn closed(&self, index: usize) -> bool {
        self.watches.borrow()[index].1.get()
    }
}

/// One recorded watch, closed through either route.
struct RecordedWatch {
    /// Whether the watch was closed.
    closed: Rc<Cell<bool>>,
}

impl FsWatcher for RecordedWatch {
    fn close(&mut self) -> io::Result<()> {
        self.closed.set(true);
        Ok(())
    }
}

impl Drop for RecordedWatch {
    fn drop(&mut self) {
        self.closed.set(true);
    }
}

/// A scheduled callback that never runs.
struct IdleTimer;

impl WatchTimer for IdleTimer {
    fn cancel(&mut self) {}
}

impl WatchOperations for RecordingWatcher {
    fn watch(
        &self,
        path: &str,
        _listener: Rc<dyn Fn(Option<String>)>,
        _on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn FsWatcher>> {
        let closed = Rc::new(Cell::new(false));
        self.watches
            .borrow_mut()
            .push((path.to_owned(), Rc::clone(&closed)));
        Ok(Box::new(RecordedWatch { closed }))
    }

    fn schedule(&self, _delay: Duration, _callback: Box<dyn FnOnce()>) -> Box<dyn WatchTimer> {
        Box::new(IdleTimer)
    }
}

impl FooterOperations for RecordingWatcher {
    fn symbolic_ref_async(
        &self,
        _repo: &str,
        callback: Box<dyn FnOnce(io::Result<Option<String>>)>,
    ) {
        callback(Ok(None));
    }

    fn watch_file(
        &self,
        _path: &str,
        _interval: Duration,
        _listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
    ) -> Box<dyn FsWatcher> {
        Box::new(RecordedWatch {
            closed: Rc::default(),
        })
    }

    fn symbolic_ref_sync(&self, _repo_dir: &str) -> io::Result<Option<String>> {
        Ok(None)
    }
}

/// The recorded theme selection.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// Select a theme by name.
    SetTheme {
        /// The selected name.
        name: String,
    },
    /// Publish a prepared theme instance.
    SetThemeInstance,
    /// List, load and publish through the theme owner.
    ThemeQueries,
    /// Replace and read the editor text, then paste.
    EditorAccess {
        /// The text the editor is given.
        text: String,
    },
    /// Set the terminal title.
    SetTitle {
        /// The supplied title.
        text: String,
    },
    /// Read and flip the tool output expansion.
    ToolExpansionForward {
        /// The expansion the recorded run started from.
        expanded: bool,
    },
}

/// The recorded theme-selection outcome.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    /// Whether the theme was applied.
    success: bool,
    /// The reported failure message.
    #[serde(default)]
    error: Option<String>,
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// A theme selection's outcome, saved names, published probe and frame requests.
    Theme {
        /// The reported outcome.
        result: Outcome,
        /// The theme names written to the settings.
        writes: Vec<String>,
        /// The error colour probed from the published theme.
        #[serde(default, rename = "publishedError")]
        published_error: Option<String>,
        /// Whether a frame was requested.
        #[serde(rename = "renderRequested")]
        render_requested: bool,
    },
    /// The listed names, missing lookup and publication state.
    Queries {
        /// The listed theme names.
        names: Vec<String>,
        /// The missing name's lookup.
        missing: Option<String>,
        /// Whether a lookup published nothing.
        #[serde(rename = "lookupDoesNotPublish")]
        lookup_does_not_publish: bool,
        /// Whether a retained live view follows a later switch.
        #[serde(rename = "retainedLivePublishes")]
        retained_live_publishes: bool,
    },
    /// The editor text after setting, pasting and reading it back.
    Editor {
        /// The text the editor was given.
        set: String,
        /// The text read through the port after the paste.
        pasted: String,
        /// The editor's own text after the paste.
        raw: String,
    },
    /// The title the terminal received.
    Title {
        /// The supplied title.
        title: String,
    },
    /// The expansion before and after the port's selection.
    Expansion {
        /// The expansion the recorded run started from.
        before: bool,
        /// The expansion read through the port after the selection.
        selected: Vec<bool>,
    },
}

#[test]
fn named_theme_switch_persists_before_redraw() {
    let test = "named_theme_switch_persists_before_redraw";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::SetTheme { name },
                Expected::Theme {
                    result,
                    writes,
                    render_requested,
                    ..
                },
            ) = (input, expected)
            else {
                panic!("a theme selection case");
            };
            let world = started(directory).await;
            let (storage, receiver) = recording_storage("dark");
            record_settings(&world, &storage);
            let host = port(&world.mode);
            let selected = host.get_theme(name.clone()).unwrap().unwrap();
            let outcome = host.set_theme(ThemeSelection::Name(name)).unwrap();
            assert_eq!(
                Outcome {
                    success: outcome.success,
                    error: outcome.error,
                },
                result
            );
            assert_eq!(saved_themes(&world, &storage, &receiver), writes);
            assert_eq!(world.runtime.pending() > 0, render_requested);
            assert_repainted(&world, &selected, &dark());
        }
    });
}

#[test]
fn unchanged_theme_does_not_enqueue_a_setting_write() {
    let test = "unchanged_theme_does_not_enqueue_a_setting_write";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::SetTheme { name },
                Expected::Theme {
                    result,
                    writes,
                    render_requested,
                    ..
                },
            ) = (input, expected)
            else {
                panic!("a theme selection case");
            };
            let world = started(directory).await;
            let (storage, receiver) = recording_storage("dark");
            record_settings(&world, &storage);
            let outcome = port(&world.mode)
                .set_theme(ThemeSelection::Name(name))
                .unwrap();
            assert_eq!(
                Outcome {
                    success: outcome.success,
                    error: outcome.error,
                },
                result
            );
            // An unchanged selection leaves no theme edit ahead of the barrier write.
            assert_eq!(saved_themes(&world, &storage, &receiver), writes);
            assert_eq!(world.runtime.pending() > 0, render_requested);
        }
    });
}

#[test]
fn theme_rejection_does_not_persist_or_redraw() {
    let test = "theme_rejection_does_not_persist_or_redraw";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::SetTheme { name },
                Expected::Theme {
                    result,
                    writes,
                    render_requested,
                    ..
                },
            ) = (input, expected)
            else {
                panic!("a theme selection case");
            };
            let world = Setup {
                directories: Some(theme_dirs(directory)),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            let (storage, receiver) = recording_storage("dark");
            record_settings(&world, &storage);
            let outcome = port(&world.mode)
                .set_theme(ThemeSelection::Name(name))
                .unwrap();
            assert_eq!(
                Outcome {
                    success: outcome.success,
                    error: outcome.error,
                },
                result
            );
            assert_eq!(saved_themes(&world, &storage, &receiver), writes);
            assert!(!render_requested);
            assert_eq!(world.runtime.pending(), 0, "a rejected name adds no frame");
        }
    });
}

#[test]
fn theme_instances_publish_without_changing_the_saved_name() {
    let test = "theme_instances_publish_without_changing_the_saved_name";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::SetThemeInstance,
                Expected::Theme {
                    result,
                    writes,
                    published_error,
                    render_requested,
                },
            ) = (input, expected)
            else {
                panic!("a theme instance case");
            };
            let world = started(directory).await;
            let (storage, receiver) = recording_storage("dark");
            record_settings(&world, &storage);
            let instance = light();
            let outcome = port(&world.mode)
                .set_theme(ThemeSelection::Theme(LiveTheme::from_theme(Rc::clone(
                    &instance,
                ))))
                .unwrap();
            assert_eq!(
                Outcome {
                    success: outcome.success,
                    error: outcome.error,
                },
                result
            );
            assert_eq!(saved_themes(&world, &storage, &receiver), writes);
            let theme = world.themes.theme().get().unwrap();
            let probe = format!(
                "{}probe\x1b[39m",
                theme.get_fg_ansi(&ThemeColor::Error).unwrap()
            );
            assert_eq!(probe, roles(&instance, &published_error.unwrap()));
            assert_eq!(world.runtime.pending() > 0, render_requested);
            assert_repainted(&world, &instance, &dark());
        }
    });
}

#[test]
fn theme_queries_keep_lookup_separate_from_live_publication() {
    let test = "theme_queries_keep_lookup_separate_from_live_publication";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::ThemeQueries,
                Expected::Queries {
                    names,
                    missing,
                    lookup_does_not_publish,
                    retained_live_publishes,
                },
            ) = (input, expected)
            else {
                panic!("a theme query case");
            };
            let world = registered(directory).await;
            let host = port(&world.mode);
            let listed: Vec<ThemeInfo> = host.get_all_themes().unwrap();
            let source = |name: &str| {
                let file = directory.join("themes").join(format!("{name}.json"));
                Some(file.to_string_lossy().into_owned())
            };
            assert_eq!(
                listed
                    .into_iter()
                    .map(|info| (info.name, info.path))
                    .collect::<Vec<_>>(),
                names
                    .iter()
                    .map(|name| (name.clone(), source(name)))
                    .collect::<Vec<_>>(),
                "each listed theme names its source document"
            );
            let probe = error_escape(&dark());
            let retained = host.theme();
            assert_eq!(error_escape(&retained.get().unwrap()), probe);
            let absent = host.get_theme("__missing_theme__".to_owned()).unwrap();
            assert_eq!(absent.is_some(), missing.is_some());
            let loaded = host.get_theme("light".to_owned()).unwrap().unwrap();
            if lookup_does_not_publish {
                assert_eq!(
                    error_escape(&world.themes.theme().get().unwrap()),
                    probe,
                    "loading light published nothing"
                );
            }
            assert_eq!(error_escape(&loaded), error_escape(&light()));
            host.set_theme(ThemeSelection::Name("light".to_owned()))
                .unwrap();
            if retained_live_publishes {
                assert_eq!(
                    error_escape(&retained.get().unwrap()),
                    error_escape(&light()),
                    "the retained live view follows the switch"
                );
            }
        }
    });
}

#[test]
fn named_theme_switch_reuses_the_supplied_watch_owner() {
    isolated(
        "named_theme_switch_reuses_the_supplied_watch_owner",
        async |directory| {
            let directories = with_custom_theme(directory);
            let custom_dir = directories.custom_themes_dir.clone();
            let watcher = Rc::new(RecordingWatcher::default());
            let world = Setup {
                directories: Some(directories),
                watcher: Some(Rc::clone(&watcher) as Rc<dyn FooterOperations>),
                configure: Box::new(|settings| settings.set_theme("custom".to_owned())),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            assert_eq!(watcher.paths(), vec![custom_dir.clone()]);
            assert!(!watcher.closed(0), "the chosen custom theme is watched");
            let host = port(&world.mode);
            host.set_theme(ThemeSelection::Name("custom".to_owned()))
                .unwrap();
            assert_eq!(watcher.paths(), vec![custom_dir.clone(), custom_dir]);
            assert!(watcher.closed(0), "the replaced watch is closed");
            assert!(!watcher.closed(1));
            world.runtime.settle().unwrap();
            host.set_theme(ThemeSelection::Theme(LiveTheme::from_theme(light())))
                .unwrap();
            assert!(watcher.closed(1), "a direct instance stops the named watch");
            assert_eq!(watcher.paths().len(), 2, "no third registration");
        },
    );
}

#[test]
fn editor_context_uses_bracketed_paste_and_expanded_text() {
    let test = "editor_context_uses_bracketed_paste_and_expanded_text";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (Input::EditorAccess { text }, Expected::Editor { set, pasted, raw }) =
                (input, expected)
            else {
                panic!("an editor access case");
            };
            let world = started(directory).await;
            let host = port(&world.mode);
            host.set_editor_text(text.clone()).unwrap();
            assert_eq!(host.get_editor_text().unwrap(), set);
            host.paste_to_editor(PASTED.to_owned()).unwrap();
            assert_eq!(host.get_editor_text().unwrap(), pasted, "the expanded text");
            assert_eq!(world.mode.editor.get_text(), raw, "the raw text");
            assert_ne!(pasted, raw, "each getter is observed apart");
        }
    });
}

#[test]
fn terminal_title_forwards_the_supplied_text() {
    let test = "terminal_title_forwards_the_supplied_text";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (Input::SetTitle { text }, Expected::Title { title }) = (input, expected) else {
                panic!("a title case");
            };
            let world = started(directory).await;
            let written = Rc::new(RefCell::new(Vec::new()));
            let sink = Rc::clone(&written);
            world
                .terminal
                .set_tap(move |data| sink.borrow_mut().push(data.to_owned()));
            port(&world.mode).set_title(text).unwrap();
            assert_eq!(
                *written.borrow(),
                vec![format!("\x1b]0;{title}\x07")],
                "the supplied text reached the terminal unchanged"
            );
        }
    });
}

#[test]
fn ui_tool_expansion_reaches_the_existing_transcript_owner() {
    let test = "ui_tool_expansion_reaches_the_existing_transcript_owner";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::ToolExpansionForward { expanded },
                Expected::Expansion { before, selected },
            ) = (input, expected)
            else {
                panic!("an expansion case");
            };
            assert_eq!(expanded, before);
            let world = started(directory).await;
            world.mode.tool_output_expanded.set(before);
            // The walk must reach the tool row the mode installs; its own paint comes from
            // its renderer, which keeps only the tail of the output while collapsed.
            let row = world
                .mode
                .render_tool_call("row", "bash", Some(json!({"command": "seq 40"})))
                .unwrap();
            row.update_result(output(&lines()), false).unwrap();
            let shows_first = |row: &crate::ToolExecutionComponent| {
                row.render(80).iter().any(|line| line.contains("row 0"))
            };
            assert_eq!(shows_first(&row), before, "the row starts as the flag says");
            // The expandable resource row paints the expansion the walk applies.
            let expandable = Rc::new(
                crate::interactive_mode::startup::ExpandableText::new(
                    Box::new(|| Ok("SHORT".to_owned())),
                    Box::new(|| Ok("LONG".to_owned())),
                    before,
                    0,
                    0,
                )
                .unwrap(),
            );
            world.mode.chat_container.add_child(expandable.clone());
            assert_eq!(
                painted_rows(&expandable),
                vec![if before { "LONG" } else { "SHORT" }]
            );
            let host = port(&world.mode);
            assert_eq!(host.get_tools_expanded().unwrap(), before);
            host.set_tools_expanded(!before).unwrap();
            assert_eq!(host.get_tools_expanded().unwrap(), selected[0]);
            assert_eq!(world.mode.tool_output_expanded.get(), selected[0]);
            assert_eq!(
                shows_first(&row),
                selected[0],
                "the installed tool row repainted with the expansion"
            );
            assert_eq!(
                painted_rows(&expandable),
                vec![if before { "SHORT" } else { "LONG" }],
                "the retained transcript row changed"
            );
            assert!(world.runtime.pending() > 0);
        }
    });
}

#[test]
fn ui_notice_and_tool_failures_keep_their_original_error_types() {
    isolated(
        "ui_notice_and_tool_failures_keep_their_original_error_types",
        async |directory| {
            let world = started(directory).await;
            let row =
                Rc::new(crate::ToolExecutionComponent::new(tool_params(&world, "row")).unwrap());
            row.update_result(output("one"), false).unwrap();
            world.mode.chat_container.add_child(row);
            let empty =
                Rc::new(Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap());
            world.themes.set_theme_instance(empty).unwrap();
            let host = port(&world.mode);
            let children = world.mode.chat_container.children().borrow().len();
            for kind in [
                None,
                Some(maestro_app::NotificationType::Warning),
                Some(maestro_app::NotificationType::Error),
            ] {
                let error = host.notify("refused".to_owned(), kind).unwrap_err();
                assert!(error.downcast_ref::<maestro_theme::ThemeError>().is_some());
                assert_eq!(
                    world.mode.chat_container.children().borrow().len(),
                    children
                );
            }
            let error = host.set_tools_expanded(true).unwrap_err();
            let diagnostic = error
                .downcast_ref::<DiagnosticErrorInfo>()
                .expect("the tool row's own diagnostic");
            assert!(
                diagnostic.message.contains("Unknown theme color"),
                "{diagnostic:?}"
            );
            assert!(
                world.mode.tool_output_expanded.get(),
                "the flag change precedes the row failure"
            );
        },
    );
}

#[test]
fn retained_ui_context_keeps_the_mode_until_it_is_released() {
    isolated(
        "retained_ui_context_keeps_the_mode_until_it_is_released",
        async |directory| {
            let world = started(directory).await;
            let mode = Rc::downgrade(&world.mode);
            let held = port(&world.mode);
            drop(world);
            assert!(
                mode.upgrade().is_some(),
                "the retained context keeps the mode"
            );
            held.set_title("still served".to_owned()).unwrap();
            drop(held);
            assert!(
                mode.upgrade().is_none(),
                "the released context frees the mode"
            );
        },
    );
}

/// The row's rendered lines without their trailing padding.
fn painted_rows(row: &crate::interactive_mode::startup::ExpandableText) -> Vec<String> {
    row.render(80)
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

/// Output long enough that collapsing it hides rows.
fn lines() -> String {
    (0..40)
        .map(|index| format!("row {index}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The result of a tool call whose output spans `text`.
fn output(text: &str) -> crate::ToolExecutionResult {
    crate::ToolExecutionResult {
        content: vec![UserBlock::Text(TextContent::new(text.to_owned()))],
        details: None,
        is_error: false,
    }
}
