//! Authored editor factories reach the production runtime and the native editor operations.
#![cfg(test)]

use maestro_extensions::{
    EventBusController, EventError, ExtensionRunner, ExtensionUIContext, Local,
    create_event_bus_for,
    types::{EditorFactory, EditorFactoryOutcome, ExtensionUIContextPort},
};
use maestro_tui::{EditorComponent, KeybindingDefinition, KeybindingKeys, KeybindingsManager};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The application's editor slot: it keeps the factory and builds the editor at once.
struct Ui {
    /// Original terminal factories receive.
    tui: maestro_tui::TUI,
    /// The published live theme.
    theme: maestro_theme::LiveTheme,
    /// Keybindings that bind the actions an editor can run.
    keybindings: KeybindingsManager,
    /// The current factory.
    factory: RefCell<Option<EditorFactory>>,
    /// The editor the current factory built.
    built: RefCell<Option<EditorFactoryOutcome>>,
}

impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        self.theme.clone()
    }
    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        self.factory.replace(factory.clone());
        let built = factory
            .map(|factory| {
                let theme = maestro_theme::get_editor_theme(self.theme());
                factory.call((self.tui.clone(), theme, self.keybindings.clone()))
            })
            .transpose()?;
        self.built.replace(built);
        Ok(())
    }
    fn get_editor_component(&self) -> Result<Option<EditorFactory>, EventError> {
        Ok(self.factory.borrow().clone())
    }
}

/// A production runner, the application's editor slot and what native callbacks received.
struct Fixture {
    /// The original native event bus.
    bus: EventBusController<Local>,
    /// The production component runner.
    runner: ExtensionRunner<Local>,
    /// The application's editor slot.
    ui: Rc<Ui>,
    /// Native text callbacks and authored emissions, in order.
    calls: Rc<RefCell<Vec<String>>>,
}

impl Fixture {
    /// Loads the real authored component through the production artifact factory.
    async fn new() -> Self {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let calls = Rc::new(RefCell::new(Vec::new()));
        for channel in ["custom-action", "modal-kept"] {
            let calls = calls.clone();
            bus.on(
                channel,
                Rc::new(move |value, _| {
                    calls.borrow_mut().push(format!("{channel} {value}"));
                    Ok(None)
                }),
            );
        }
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = super::canonical_ui::screen();
        let definition = |key: &str| KeybindingDefinition {
            default_keys: KeybindingKeys::Single(key.into()),
            description: None,
        };
        let keybindings = KeybindingsManager::new(
            vec![
                ("app.demo".into(), definition("ctrl+x")),
                ("app.host".into(), definition("ctrl+y")),
            ],
            Vec::new(),
        );
        let ui = Rc::new(Ui {
            tui,
            theme: published(),
            keybindings,
            factory: RefCell::default(),
            built: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        Self {
            bus,
            runner,
            ui,
            calls,
        }
    }

    /// Installs the authored editor `kind` names and returns its report.
    async fn install(&self, kind: &str) -> String {
        let handler = self
            .runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        handler(
            format!("ui editor {kind}"),
            self.runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string()
    }

    /// The editor the current factory built.
    fn built(&self) -> EditorFactoryOutcome {
        self.ui.built.borrow().clone().unwrap()
    }

    /// A native text callback that records what it receives under `name`.
    fn recorder(&self, name: &'static str) -> maestro_tui::editor_component::TextCallback {
        let calls = self.calls.clone();
        Rc::new(move |text: &str| calls.borrow_mut().push(format!("{name} {}", json!(text))))
    }

    /// Everything recorded so far, cleared.
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }
}

/// A live theme with a published theme instance.
fn published() -> maestro_theme::LiveTheme {
    let state = maestro_theme::ThemeState::new(maestro_theme::ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    let theme = maestro_theme::Theme::new(
        [],
        [],
        maestro_theme::ColorMode::Truecolor,
        maestro_theme::ThemeOptions::default(),
    );
    state.set_theme_instance(Rc::new(theme.unwrap())).unwrap();
    state.theme()
}

/// Delivers input to a native editor and reports its failure.
fn input(editor: &dyn EditorComponent<Signal = maestro_tui::Cancellation>, data: &str) {
    editor.try_handle_input(data).unwrap();
}

/// A completion provider with no suggestions.
struct Silent;

/// The suggestion future of a native provider.
type Pending<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<
                    Option<maestro_tui::AutocompleteSuggestions>,
                    maestro_tui::autocomplete::CompletionError,
                >,
            > + 'a,
    >,
>;

impl maestro_tui::AutocompleteProvider for Silent {
    type Signal = maestro_tui::Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        _: &'a [String],
        _: maestro_tui::autocomplete::CursorPosition,
        _: maestro_tui::autocomplete::CompletionOptions<'a, maestro_tui::Cancellation>,
    ) -> Pending<'a> {
        Box::pin(std::future::ready(Ok(None)))
    }
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: maestro_tui::autocomplete::CursorPosition,
        _: &maestro_tui::AutocompleteItem,
        _: &str,
    ) -> maestro_tui::autocomplete::CompletionResult {
        maestro_tui::autocomplete::CompletionResult {
            lines: lines.to_vec(),
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        }
    }
}

/// Each optional operation of a plain authored editor is absent, not a no-op.
fn absent(editor: &dyn EditorComponent<Signal = maestro_tui::Cancellation>) {
    let provider = Rc::new(Silent);
    assert_eq!(
        editor.try_set_autocomplete_provider(provider).unwrap(),
        None
    );
    assert_eq!(editor.try_get_expanded_text().unwrap(), None);
    assert!(editor.border_color().is_none());
    assert_eq!(editor.set_border_color(Rc::new(str::to_owned)), None);
    assert_eq!(editor.try_set_padding_x(2.0).unwrap(), None);
    assert_eq!(editor.try_add_to_history("past").unwrap(), None);
    assert_eq!(editor.try_insert_text_at_cursor("ins").unwrap(), None);
    assert_eq!(editor.try_set_autocomplete_max_visible(3.0).unwrap(), None);
}

/// An authored editor keeps native callbacks and text, reports optional operations absent and
/// returns a callable change handler.
async fn plain(fixture: &Fixture) {
    assert_eq!(
        fixture.install("plain").await,
        json!({ "same": true }).to_string()
    );
    let built = fixture.built();
    assert!(built.as_custom().is_none());
    let editor = built.editor();
    editor.set_on_submit(Some(fixture.recorder("submit")));
    editor.set_on_change(Some(fixture.recorder("change")));
    editor.try_set_text("λ\n雪").unwrap();
    assert_eq!(editor.try_get_text().unwrap(), "λ\n雪");
    input(editor.as_ref(), "!");
    input(editor.as_ref(), "enter");
    assert_eq!(editor.try_render(9).unwrap(), ["plain|9", "λ", "雪!"]);
    assert!(editor.on_submit().is_some());
    absent(editor.as_ref());
    editor.on_change().unwrap()("probe");
    editor.set_on_change(None);
    assert!(editor.on_change().is_none());
    editor.try_set_text("after").unwrap();
    let calls = [
        r#"change "λ\n雪""#,
        r#"change "λ\n雪!""#,
        r#"submit "λ\n雪!""#,
        r#"change "probe""#,
    ];
    assert_eq!(fixture.take(), calls);
}

/// The host's editor answers every optional operation through the authored transport.
async fn host(fixture: &Fixture) {
    assert_eq!(
        fixture.install("host").await,
        json!({ "same": true }).to_string()
    );
    let editor = fixture.built().editor();
    editor.set_on_change(Some(fixture.recorder("change")));
    editor.try_set_text("a").unwrap();
    assert_eq!(
        editor.try_get_expanded_text().unwrap().as_deref(),
        Some("a")
    );
    let border = editor.border_color().unwrap();
    let theme = maestro_theme::get_editor_theme(fixture.ui.theme());
    assert_eq!(border("b"), (theme.border_color)("b"));
    assert_eq!(
        editor.set_border_color(Rc::new(|text: &str| format!("<{text}>"))),
        Some(())
    );
    assert_eq!(editor.border_color().unwrap()("b"), "<b>");
    assert_eq!(editor.try_set_padding_x(2.0).unwrap(), Some(()));
    assert_eq!(editor.try_add_to_history("past").unwrap(), Some(()));
    assert_eq!(editor.try_insert_text_at_cursor("!").unwrap(), Some(()));
    assert_eq!(
        editor.try_set_autocomplete_max_visible(4.0).unwrap(),
        Some(())
    );
    let provider = Rc::new(Silent);
    assert_eq!(
        editor.try_set_autocomplete_provider(provider).unwrap(),
        Some(())
    );
    assert_eq!(editor.try_get_text().unwrap(), "a!");
    editor.on_change().unwrap()("probe");
    assert_eq!(
        fixture.take(),
        [r#"change "a""#, r#"change "a!""#, r#"change "probe""#]
    );
}

/// An authored editor on the custom editor shares that editor's actions, keeps the
/// application's handler and passes on the input it does not keep.
async fn custom(fixture: &Fixture) {
    assert_eq!(
        fixture.install("custom").await,
        json!({ "same": true }).to_string()
    );
    let built = fixture.built();
    let custom = built.as_custom().unwrap();
    let actions = custom.action_handlers();
    let names: Vec<_> = actions.borrow().keys().cloned().collect();
    assert_eq!(names, ["app.demo"]);
    let host_calls = fixture.calls.clone();
    let replaced = actions.borrow_mut().insert(
        "app.host".into(),
        Rc::new(move || host_calls.borrow_mut().push("host action".into())),
    );
    assert!(replaced.is_none());
    assert!(custom.on_escape().is_none());
    custom.set_on_escape(Some(Rc::new(|| {})));
    assert!(custom.on_escape().is_some());
    let editor = built.editor();
    for data in ["x", "q", "\u{18}", "\u{19}"] {
        input(editor.as_ref(), data);
    }
    assert_eq!(editor.try_get_text().unwrap(), "q");
    assert_eq!(editor.try_set_padding_x(1.0).unwrap(), Some(()));
    assert!(editor.focusable().is_some());
    let demo = actions.borrow()["app.demo"].clone();
    demo();
    let calls = [
        r#"modal-kept "x""#,
        r#"custom-action "app.demo""#,
        "host action",
        r#"custom-action "app.demo""#,
    ];
    assert_eq!(fixture.take(), calls);
}

#[test]
fn canonical_editor_factories_keep_callbacks_text_and_custom_actions() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let fixture = Fixture::new().await;
        assert_eq!(
            fixture.install("none").await,
            json!({ "current": false }).to_string()
        );
        plain(&fixture).await;
        host(&fixture).await;
        custom(&fixture).await;
        assert_eq!(
            fixture.install("failing").await,
            "editor factory failed 雪\n"
        );
        let none: Value = serde_json::from_str(&fixture.install("none").await).unwrap();
        assert_eq!(none, json!({ "current": false }));
        drop(fixture.runner);
        fixture.bus.clear();
    });
}

/// An editor factory another extension sets: the toolkit editor, or the witnesses' custom
/// editor, which has none of the optional operations.
async fn native_factory(fixture: &Fixture, custom: bool) -> EditorFactory {
    let made = Rc::new(RefCell::new(None));
    let slot = made.clone();
    let factory: maestro_extensions::ExtensionFactory<Local> = Rc::new(move |api| {
        let factory: EditorFactory = api.ui_callback(move |(tui, theme, keys), _| {
            let options = maestro_tui::EditorOptions::default();
            Ok(if custom {
                let widgets = super::support::widgets();
                EditorFactoryOutcome::CustomEditor(
                    widgets.custom_editor(&tui, theme, keys, options),
                )
            } else {
                EditorFactoryOutcome::Editor(Rc::new(maestro_tui::Editor::new(
                    &tui, theme, options,
                )))
            })
        });
        slot.replace(Some(factory));
        Box::pin(async { Ok(()) })
    });
    maestro_extensions::load_extension_from_factory(
        factory,
        "/other".into(),
        fixture.bus.for_extension(),
        maestro_extensions::create_extension_runtime::<Local>(),
        Some("<other>".into()),
    )
    .await
    .unwrap();
    made.take().unwrap()
}

/// What each optional operation of an editor answers, in order.
fn optional(editor: &dyn EditorComponent<Signal = maestro_tui::Cancellation>) -> Vec<bool> {
    vec![
        editor.try_insert_text_at_cursor("!").unwrap().is_some(),
        editor.try_get_expanded_text().unwrap().is_some(),
        editor.border_color().is_some(),
        editor
            .set_border_color(Rc::new(|text: &str| format!("<{text}>")))
            .is_some(),
        editor.try_set_padding_x(2.0).unwrap().is_some(),
        editor.try_add_to_history("past").unwrap().is_some(),
        editor
            .try_set_autocomplete_max_visible(4.0)
            .unwrap()
            .is_some(),
        editor
            .try_set_autocomplete_provider(Rc::new(Silent))
            .unwrap()
            .is_some(),
    ]
}

/// An authored factory wrapping another extension's builds that extension's editor through the
/// host, keeping its text, callbacks, custom actions and absent optional operations.
async fn foreign(fixture: &Fixture, custom: bool) {
    let native = native_factory(fixture, custom).await;
    fixture.ui.set_editor_component(Some(native)).unwrap();
    assert_eq!(
        fixture.install("wrap").await,
        json!({ "same": true }).to_string()
    );
    let built = fixture.built();
    assert_eq!(built.as_custom().is_some(), custom);
    let editor = built.editor();
    editor.set_on_change(Some(fixture.recorder("change")));
    editor.try_set_text("a").unwrap();
    assert_eq!(optional(editor.as_ref()), [!custom; 8]);
    input(editor.as_ref(), "q");
    let typed = if custom { "aq" } else { "a!q" };
    assert_eq!(editor.try_get_text().unwrap(), typed);
    editor.on_change().unwrap()("probe");
    let mut calls = vec![r#"change "a""#.to_owned()];
    calls.extend((!custom).then(|| r#"change "a!""#.to_owned()));
    calls.extend([
        format!("change {}", json!(typed)),
        r#"change "probe""#.to_owned(),
    ]);
    assert_eq!(fixture.take(), calls);
    if let Some(custom) = built.as_custom() {
        assert!(custom.on_escape().is_none());
        custom.set_on_escape(Some(Rc::new(|| {})));
        assert!(custom.on_escape().is_some());
    }
    let none: Value = serde_json::from_str(&fixture.install("none").await).unwrap();
    assert_eq!(none, json!({ "current": false }));
}

#[test]
fn canonical_editor_factories_wrap_another_extensions_factory() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let fixture = Fixture::new().await;
        assert_eq!(fixture.install("wrap").await, "no editor factory to wrap");
        foreign(&fixture, false).await;
        foreign(&fixture, true).await;
        drop(fixture.runner);
        fixture.bus.clear();
    });
}
