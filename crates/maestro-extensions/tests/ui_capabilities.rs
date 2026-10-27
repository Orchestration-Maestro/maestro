//! Retained UI capabilities, headless results and typed message renderers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::error_support;
use crate::limited_host;
use crate::local_support;
#[path = "../../maestro-theme/tests/published/mod.rs"]
mod published;
use crate::ui_host;
use crate::ui_toolkit;
use ui_host::Named;

use local_support::{
    Loaded, LocalBus, Released, STALE, Trace, listener, loaded, local, mark, runner,
};
use maestro_extensions::footer_data_provider::{ExtensionStatuses, ReadonlyFooterDataProvider};
use maestro_extensions::types::{
    ComponentDisposer, ComponentFactory, CustomClose, CustomComponentFactory, CustomDone,
    CustomFactory, CustomFactoryOutcome, CustomUIOptions, EditorFactoryOutcome,
    ExtensionCollection, ExtensionComponent, ExtensionKeybindings, ExtensionShortcutOptions,
    ExtensionUIContextPort, ExtensionUIFuture, FooterFactory, NotificationType,
    OverlayHandleCallback, OverlayOptionsResolver, OverlayOptionsSource, RegisteredCommandOptions,
    ThemeSelection, ThemeSetResult, WidgetContent,
};
use maestro_extensions::{
    AutocompleteProviderFactory, CallbackEmitter, EditorFactory, EventError, ExtensionAPI,
    ExtensionRunner, ExtensionUIContext, ExtensionUIContextBinding, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer, ReplacedSessionContext,
    TerminalInputHandler, WidgetPlacement, WorkingIndicatorOptions,
};
use maestro_models::{Cancellation, Local, UserContent};
use maestro_session::Presence;
use maestro_session::messages::CustomMessage;
use maestro_theme::{LiveTheme, Theme, ThemeColor, ThemeInfo, ThemeState, get_editor_theme};
use maestro_tui::autocomplete::{
    CompletionError, CompletionOptions, CompletionResult, CursorPosition,
};
use maestro_tui::tui::{ComponentHandle, InputListenerResult};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Editor, EditorComponent,
    EditorOptions, EditorTheme, Input, KeybindingDefinition, KeybindingKeys, KeybindingsManager,
    OverlayHandle, OverlayOptions, TUI, TUI_KEYBINDINGS,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot;
use ui_toolkit::{Keyboard, Metadata, Rejecting, Words};

/// The command context every local test callback receives.
type Ui = ExtensionUIContext;

/// Dialog options with a retained signal and a supplied timeout.
fn dialog(signal: &Cancellation, timeout: Option<f64>) -> ExtensionUIDialogOptions {
    ExtensionUIDialogOptions {
        signal: Some(signal.clone()),
        timeout,
    }
}

/// Settles headless choices with no selection and confirmations with `false`.
async fn headless_choices(ui: &Ui, signal: &Cancellation) {
    for choices in [Vec::new(), vec!["b".to_owned(), "a".to_owned()]] {
        let plain = ui.select("t".into(), choices.clone(), None);
        assert_eq!(plain.await.unwrap(), None);
        let selected = ui.select("t".into(), choices, Some(dialog(signal, Some(5.0))));
        assert_eq!(selected.await.unwrap(), None);
    }
    for options in [None, Some(dialog(signal, None))] {
        assert!(!ui.confirm("t".into(), "m".into(), options).await.unwrap());
    }
}

/// Settles headless text dialogs with no text.
async fn headless_text(ui: &Ui, signal: &Cancellation) {
    for text in [None, Some(String::new()), Some("p".to_owned())] {
        let input = ui.input("t".into(), text.clone(), Some(dialog(signal, Some(0.0))));
        assert_eq!(input.await.unwrap(), None);
        assert_eq!(ui.editor("t".into(), text).await.unwrap(), None);
    }
}

/// Settles headless custom components without calling their callbacks.
async fn headless_custom(ui: &Ui, rejected: &Rejecting) {
    let options = CustomUIOptions {
        overlay: Some(true),
        overlay_options: Some(OverlayOptionsSource::Resolver(rejected.resolver())),
        on_handle: Some(rejected.on_handle()),
    };
    assert_eq!(
        ui.custom(rejected.custom(), Some(options)).await.unwrap(),
        None
    );
    let options = CustomUIOptions {
        overlay: Some(false),
        overlay_options: Some(OverlayOptionsSource::Static(Rc::default())),
        on_handle: None,
    };
    assert_eq!(
        ui.custom(rejected.custom(), Some(options)).await.unwrap(),
        None
    );
}

/// Applies every headless control without effect.
fn headless_controls(ui: &Ui, rejected: &Rejecting) {
    for kind in [
        None,
        Some(NotificationType::Info),
        Some(NotificationType::Warning),
    ] {
        ui.notify("n".into(), kind).unwrap();
    }
    ui.notify("n".into(), Some(NotificationType::Error))
        .unwrap();
    let unsubscribe = ui.on_terminal_input(rejected.input()).unwrap();
    unsubscribe().unwrap();
    unsubscribe().unwrap();
    for text in [None, Some(String::new()), Some("x".to_owned())] {
        ui.set_status("k".into(), text.clone()).unwrap();
        ui.set_working_message(text.clone()).unwrap();
        ui.set_hidden_thinking_label(text).unwrap();
    }
    for visible in [false, true] {
        ui.set_working_visible(visible).unwrap();
    }
    for frames in [None, Some(Vec::new()), Some(vec!["●".to_owned()])] {
        ui.set_working_indicator(Some(WorkingIndicatorOptions {
            frames,
            interval_ms: Some(80.5),
        }))
        .unwrap();
    }
    ui.set_working_indicator(None).unwrap();
    for placement in [
        None,
        Some(WidgetPlacement::AboveEditor),
        Some(WidgetPlacement::BelowEditor),
    ] {
        let options = Some(ExtensionWidgetOptions { placement });
        let lines = Some(WidgetContent::Lines(vec!["line".into()]));
        ui.set_widget("w".into(), lines, options).unwrap();
        let factory = Some(WidgetContent::Factory(rejected.component()));
        ui.set_widget("w".into(), factory, None).unwrap();
        ui.set_widget("w".into(), None, options).unwrap();
    }
    ui.set_footer(Some(rejected.footer())).unwrap();
    ui.set_footer(None).unwrap();
    ui.set_header(Some(rejected.component())).unwrap();
    ui.set_header(None).unwrap();
    ui.set_title(String::new()).unwrap();
    ui.set_title("標題\u{feff}\u{85}".into()).unwrap();
}

/// Reads every headless editor, autocomplete, theme and expansion result.
fn headless_queries(ui: &Ui, rejected: &Rejecting) {
    ui.paste_to_editor("a\nb\x1b[A".into()).unwrap();
    ui.set_editor_text("text".into()).unwrap();
    assert_eq!(ui.get_editor_text().unwrap(), "");
    ui.add_autocomplete_provider(rejected.autocomplete())
        .unwrap();
    ui.set_editor_component(Some(rejected.editor())).unwrap();
    assert!(ui.get_editor_component().unwrap().is_none());
    ui.set_editor_component(None).unwrap();
    assert!(ui.get_editor_component().unwrap().is_none());
    assert!(ui.get_all_themes().unwrap().is_empty());
    for name in ["", "unknown", "テーマ\u{feff}"] {
        assert!(ui.get_theme(name.into()).unwrap().is_none());
    }
    let unavailable = ThemeSetResult {
        success: false,
        error: Some("UI not available".into()),
    };
    let instance = LiveTheme::from_theme(published::theme(0));
    for selection in [
        ThemeSelection::Name("dark".into()),
        ThemeSelection::Theme(instance),
    ] {
        assert_eq!(ui.set_theme(selection).unwrap(), unavailable);
    }
    for expanded in [true, false] {
        ui.set_tools_expanded(expanded).unwrap();
        assert!(!ui.get_tools_expanded().unwrap());
    }
}

#[test]
fn no_ui_methods_return_their_defined_fallbacks() {
    local(async {
        let owners: ExtensionCollection<Local> = Rc::new(RwLock::new(Vec::new()));
        let ui = runner(&owners).get_ui_context();
        let rejected = Rejecting::new(Trace::default());
        let signal = Cancellation::new();
        headless_choices(&ui, &signal).await;
        headless_text(&ui, &signal).await;
        assert!(!signal.is_aborted());
        headless_custom(&ui, &rejected).await;
        headless_controls(&ui, &rejected);
        headless_queries(&ui, &rejected);
        assert!(rejected.trace.borrow().is_empty());
    });
}

/// Registers a shortcut on a reserved effective key and returns its runner.
fn reserved_shortcut(warnings: &Trace) -> (Loaded, ExtensionRunner<Local>) {
    let bus = LocalBus::new();
    let owner = loaded("/reserved", bus.controller.for_extension());
    let options = ExtensionShortcutOptions::<Local> {
        description: None,
        handler: Rc::new(|_| Box::pin(async { Ok(()) })),
    };
    owner
        .api
        .register_shortcut("Ctrl+C".into(), options)
        .unwrap();
    let warnings = warnings.clone();
    let runner = ExtensionRunner::<Local>::new(
        owner.owners.clone(),
        owner.runtime.clone(),
        local_support::session::runner_session("/work").0,
        Rc::new(move |line: &str| mark(&warnings, line)),
        LiveTheme::default(),
    );
    (owner, runner)
}

#[test]
fn installed_ui_presence_does_not_claim_mode_support() {
    let warnings = Trace::default();
    let (_owner, runner) = reserved_shortcut(&warnings);
    let bindings = vec![(
        "app.interrupt".to_owned(),
        Some(KeybindingKeys::Single("ctrl+c".into())),
    )];
    let warning =
        "Extension shortcut 'Ctrl+C' from /reserved conflicts with built-in shortcut. Skipping.\n";
    assert!(!runner.has_ui());
    assert!(runner.get_shortcuts(&bindings).is_empty());
    assert_eq!(*warnings.borrow(), [warning]);
    runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(
        limited_host::Limited,
    ))));
    assert!(runner.has_ui());
    assert_eq!(runner.get_ui_context().get_editor_text().unwrap(), "");
    assert!(runner.get_shortcuts(&bindings).is_empty());
    assert_eq!(runner.get_shortcut_diagnostics().read().unwrap().len(), 1);
    assert_eq!(*warnings.borrow(), [warning]);
    runner.set_ui_context(None);
    assert!(!runner.has_ui());
    assert!(runner.get_shortcuts(&bindings).is_empty());
    assert_eq!(*warnings.borrow(), [warning, warning]);
    let fallback = runner.get_ui_context();
    runner.set_ui_context(Some(fallback.clone()));
    assert!(!runner.has_ui());
    let owners: ExtensionCollection<Local> = Rc::new(RwLock::new(Vec::new()));
    let other = runner_with_fallback_of(&owners, fallback);
    assert!(!other.has_ui());
}

/// A second runner given another runner's retained fallback.
fn runner_with_fallback_of(
    owners: &ExtensionCollection<Local>,
    fallback: Ui,
) -> ExtensionRunner<Local> {
    let other = runner(owners);
    other.set_ui_context(Some(ExtensionUIContext::new(Rc::new(
        limited_host::Limited,
    ))));
    assert!(other.has_ui());
    other.set_ui_context(Some(fallback));
    other
}

/// A replacement-session context reads its own runner's host, not its creator's.
fn replacement_context_reads_its_own_runner(
    owners: &ExtensionCollection<Local>,
    creator: &ExtensionRunner<Local>,
) {
    let fresh = runner(owners);
    fresh.set_ui_context(Some(Named::install("r").0));
    let replaced = ReplacedSessionContext::new(
        fresh.create_command_context(),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
    );
    let ui = replaced.ui().unwrap();
    ui.set_editor_text("replacement".into()).unwrap();
    assert_eq!(
        creator.get_ui_context().get_editor_text().unwrap(),
        "second"
    );
    fresh.set_ui_context(None);
    assert_eq!(replaced.ui().unwrap().get_editor_text().unwrap(), "");
    assert_eq!(ui.get_editor_text().unwrap(), "replacement");
}

/// An acquired handle's aliases keep its host until the last one drops.
fn last_alias_releases_host(handle: Ui, released: &Weak<Named>) {
    let alias = handle.clone();
    drop(handle);
    assert!(released.upgrade().is_some());
    drop(alias);
    assert!(released.upgrade().is_none());
}

/// A borrowed binding serves its host only while another owner retains it.
fn borrowed_binding_does_not_retain_its_host(runner: &ExtensionRunner<Local>) {
    let (installed, host) = Named::install("c");
    let released = Rc::downgrade(&host);
    runner.set_ui_context_binding(Some(ExtensionUIContextBinding::borrowed(&installed)));
    drop(installed);
    let acquired = runner.get_ui_context();
    drop(host);
    acquired.set_editor_text("borrowed".into()).unwrap();
    assert!(runner.has_ui());
    assert_eq!(
        runner.get_ui_context().get_editor_text().unwrap(),
        "borrowed"
    );
    last_alias_releases_host(acquired, &released);
    assert!(!runner.has_ui());
    assert_eq!(runner.get_ui_context().get_editor_text().unwrap(), "");
}

#[test]
fn retained_ui_handles_keep_old_hosts_after_rebinding() {
    local(async {
        let owners: ExtensionCollection<Local> = Rc::new(RwLock::new(Vec::new()));
        let runner = runner(&owners);
        let (first, host) = Named::install("a");
        let released = Rc::downgrade(&host);
        drop(host);
        runner.set_ui_context(Some(first));
        let (event, command) = (runner.create_context(), runner.create_command_context());
        let old = event.ui().unwrap();
        let old_command = command.ui().unwrap();
        runner.set_ui_context(Some(Named::install("b").0));
        let current = runner.get_ui_context();
        old.set_editor_text("first".into()).unwrap();
        current.set_editor_text("second".into()).unwrap();
        assert_eq!(old.get_editor_text().unwrap(), "first");
        assert_eq!(old_command.get_editor_text().unwrap(), "first");
        assert_eq!(runner.get_ui_context().get_editor_text().unwrap(), "second");
        assert_eq!(event.ui().unwrap().get_editor_text().unwrap(), "second");
        assert_eq!(command.ui().unwrap().get_editor_text().unwrap(), "second");
        replacement_context_reads_its_own_runner(&owners, &runner);
        let choice = old.select("t".into(), vec!["x".into()], None).await;
        assert_eq!(choice.unwrap().as_deref(), Some("a:t:x"));
        let choice = current.select("t".into(), vec!["x".into()], None).await;
        assert_eq!(choice.unwrap().as_deref(), Some("b:t:x"));
        runner.set_ui_context(None);
        assert_eq!(runner.get_ui_context().get_editor_text().unwrap(), "");
        assert_eq!(old.get_editor_text().unwrap(), "first");
        drop(old_command);
        last_alias_releases_host(old, &released);
        borrowed_binding_does_not_retain_its_host(&runner);
    });
}

/// A dialog host recording what it receives and answering from it.
#[derive(Default)]
struct Dialogs {
    /// Received calls, in order.
    calls: Trace,
    /// Signals the host retained.
    signals: RefCell<Vec<Cancellation>>,
    /// Whether every dialog fails.
    fail: Cell<bool>,
}
impl Dialogs {
    /// Records one call with its options, retaining the signal.
    fn receive(
        &self,
        call: &str,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> Result<(), EventError> {
        let options = opts.map(|opts| {
            let signal = opts.signal.map(|signal| {
                let aborted = signal.is_aborted();
                self.signals.borrow_mut().push(signal);
                aborted
            });
            format!("signal={signal:?} timeout={:?}", opts.timeout)
        });
        mark(&self.calls, format!("{call} {options:?}"));
        if self.fail.get() {
            return Err("dialog failed".into());
        }
        Ok(())
    }
}
impl ExtensionUIContextPort for Dialogs {
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let received = self.receive(&format!("select {title} {options:?}"), opts);
        Box::pin(async move { received.map(|()| options.last().cloned()) })
    }
    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let received = self.receive(&format!("confirm {title} {message:?}"), opts);
        Box::pin(async move { received.map(|()| !message.is_empty()) })
    }
    fn input(
        &self,
        title: String,
        placeholder: Option<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let received = self.receive(&format!("input {title} {placeholder:?}"), opts);
        Box::pin(async move { received.map(|()| placeholder) })
    }
    fn editor(&self, title: String, prefill: Option<String>) -> ExtensionUIFuture<Option<String>> {
        let received = self.receive(&format!("editor {title} {prefill:?}"), None);
        Box::pin(async move { received.map(|()| prefill.map(|text| text + "!")) })
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// Runs every dialog from a registered local command across a suspension.
async fn ask(ui: Ui, signal: Cancellation, answers: Trace) -> Result<(), EventError> {
    let options = |timeout| Some(dialog(&signal, timeout));
    let first = ui.select("pick".into(), vec!["b".into(), "a".into()], options(None));
    mark(&answers, format!("{:?}", first.await?));
    tokio::task::yield_now().await;
    signal.abort();
    let empty = ui.select("pick".into(), Vec::new(), options(Some(0.0)));
    mark(&answers, format!("{:?}", empty.await?));
    mark(
        &answers,
        format!(
            "{}",
            ui.confirm("sure".into(), String::new(), options(Some(-1.0)))
                .await?
        ),
    );
    mark(
        &answers,
        format!("{}", ui.confirm("sure".into(), "m".into(), None).await?),
    );
    for (placeholder, timeout) in [(None, Some(0.5)), (Some(String::new()), None)] {
        let input = ui.input("name".into(), placeholder, options(timeout));
        mark(&answers, format!("{:?}", input.await?));
    }
    for prefill in [None, Some(String::new()), Some("x".to_owned())] {
        mark(
            &answers,
            format!("{:?}", ui.editor("edit".into(), prefill).await?),
        );
    }
    Ok(())
}

#[test]
fn ui_dialog_options_reach_the_installed_host() {
    local(async {
        let bus = LocalBus::new();
        let owner = loaded("/dialogs", bus.controller.for_extension());
        let (signal, answers) = (Cancellation::new(), Trace::default());
        let (command_signal, output) = (signal.clone(), answers.clone());
        let options = RegisteredCommandOptions::<Local> {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |_, ctx| {
                Box::pin(ask(
                    ctx.ui().unwrap(),
                    command_signal.clone(),
                    output.clone(),
                ))
            }),
        };
        owner.api.register_command("ask".into(), options).unwrap();
        let host = Rc::new(Dialogs::default());
        let runner = runner(&owner.owners);
        runner.set_ui_context(Some(ExtensionUIContext::new(host.clone())));
        let command = runner.get_command("ask").unwrap();
        (command.handler)(String::new(), runner.create_command_context())
            .await
            .unwrap();
        assert_eq!(
            *answers.borrow(),
            [
                "Some(\"a\")",
                "None",
                "false",
                "true",
                "None",
                "Some(\"\")",
                "None",
                "Some(\"!\")",
                "Some(\"x!\")",
            ]
        );
        assert_eq!(
            *host.calls.borrow(),
            [
                "select pick [\"b\", \"a\"] Some(\"signal=Some(false) timeout=None\")",
                "select pick [] Some(\"signal=Some(true) timeout=Some(0.0)\")",
                "confirm sure \"\" Some(\"signal=Some(true) timeout=Some(-1.0)\")",
                "confirm sure \"m\" None",
                "input name None Some(\"signal=Some(true) timeout=Some(0.5)\")",
                "input name Some(\"\") Some(\"signal=Some(true) timeout=None\")",
                "editor edit None None",
                "editor edit Some(\"\") None",
                "editor edit Some(\"x\") None",
            ]
        );
        assert!(host.signals.borrow().iter().all(Cancellation::is_aborted));
        host.fail.set(true);
        let error = (command.handler)(String::new(), runner.create_command_context()).await;
        assert_eq!(error.unwrap_err().to_string(), "dialog failed");
        let error = runner.get_ui_context().editor("edit".into(), None).await;
        assert_eq!(error.unwrap_err().to_string(), "dialog failed");
    });
}

/// A retained widget.
enum Widget {
    /// Lines rendered as they are.
    Lines(Vec<String>),
    /// A component built by its factory.
    Built(ExtensionComponent),
}

/// A host building widgets and headers on a real writer, with a theme owner.
struct Screen {
    /// The real writer factories receive.
    tui: TUI,
    /// The theme owner whose live view factories receive.
    state: ThemeState,
    /// The facade widget factories are bound to.
    api: ExtensionAPI<Local>,
    /// The instance registered as `custom`.
    prepared: Rc<Theme>,
    /// Received operations.
    calls: Trace,
    /// Widgets by key with their resolved placement.
    widgets: RefCell<Vec<(String, WidgetPlacement, Widget)>>,
    /// The custom header.
    header: RefCell<Option<ExtensionComponent>>,
    /// The editor text.
    editor: RefCell<String>,
}
impl Screen {
    /// A host over `tui` and `state` whose factories are bound to `api`.
    fn new(tui: TUI, state: ThemeState, api: ExtensionAPI<Local>) -> Self {
        Self {
            tui,
            state,
            api,
            prepared: published::theme(20),
            calls: Trace::default(),
            widgets: RefCell::default(),
            header: RefCell::default(),
            editor: RefCell::default(),
        }
    }
    /// Removes the widget with `key`, if any.
    fn remove_widget(&self, key: &str) -> Option<Widget> {
        let mut widgets = self.widgets.borrow_mut();
        let index = widgets.iter().position(|(retained, ..)| retained == key)?;
        Some(widgets.remove(index).2)
    }
}
impl ExtensionUIContextPort for Screen {
    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let placement = options
            .and_then(|options| options.placement)
            .unwrap_or_default();
        if let Some(Widget::Built(component)) = self.remove_widget(&key) {
            ui_toolkit::dispose(Some(component));
        }
        let theme = self.state.theme();
        let widget = match content {
            None => None,
            Some(WidgetContent::Lines(lines)) => Some(Widget::Lines(lines)),
            Some(WidgetContent::Factory(factory)) => Some(Widget::Built(ui_toolkit::build(
                &self.tui, &theme, &factory,
            )?)),
        };
        mark(
            &self.calls,
            format!("widget {key} {placement:?} {}", widget.is_some()),
        );
        if let Some(widget) = widget {
            self.widgets.borrow_mut().push((key, placement, widget));
        }
        Ok(())
    }
    fn set_header(&self, factory: Option<ComponentFactory>) -> Result<(), EventError> {
        let theme = self.state.theme();
        let header = factory
            .map(|factory| ui_toolkit::build(&self.tui, &theme, &factory))
            .transpose()?;
        let previous = self.header.replace(header);
        ui_toolkit::dispose(previous);
        Ok(())
    }
    fn set_title(&self, title: String) -> Result<(), EventError> {
        mark(&self.calls, format!("title {title}"));
        Ok(())
    }
    fn paste_to_editor(&self, text: String) -> Result<(), EventError> {
        self.editor.borrow_mut().push_str(&text);
        Ok(())
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        *self.editor.borrow_mut() = text;
        Ok(())
    }
    fn get_editor_text(&self) -> Result<String, EventError> {
        Ok(self.editor.borrow().clone())
    }
    fn theme(&self) -> LiveTheme {
        self.state.theme()
    }
    fn get_all_themes(&self) -> Result<Vec<ThemeInfo>, EventError> {
        Ok(vec![
            ThemeInfo {
                name: "dark".into(),
                path: None,
            },
            ThemeInfo {
                name: "custom".into(),
                path: Some("/themes/custom.json".into()),
            },
        ])
    }
    fn get_theme(&self, name: String) -> Result<Option<Rc<Theme>>, EventError> {
        Ok((name == "custom").then(|| Rc::clone(&self.prepared)))
    }
    fn set_theme(&self, theme: ThemeSelection) -> Result<ThemeSetResult, EventError> {
        let theme = match theme {
            ThemeSelection::Name(name) if name == "custom" => Rc::clone(&self.prepared),
            ThemeSelection::Name(name) => {
                return Ok(ThemeSetResult {
                    success: false,
                    error: Some(format!("Theme \"{name}\" not found")),
                });
            }
            ThemeSelection::Theme(theme) => theme.get()?,
        };
        self.state.set_theme_instance(theme)?;
        Ok(ThemeSetResult {
            success: true,
            error: None,
        })
    }
}

/// A factory building an input that joins the writer and records its disposal.
fn input_factory(
    api: &ExtensionAPI<Local>,
    trace: &Trace,
    label: &'static str,
) -> ComponentFactory {
    let (bind, inner, trace) = (api.clone(), api.clone(), trace.clone());
    bind.ui_callback(move |(tui, _): (TUI, LiveTheme), emitter| {
        let input = Rc::new(Input::new());
        input.set_value(label.into());
        tui.add_child(input.clone());
        mark(&trace, format!("built {label}"));
        emitter.emit("built", json!(label));
        mark(&trace, format!("returning {label}"));
        let trace = trace.clone();
        let dispose: ComponentDisposer = inner.ui_callback(move |(), _| {
            mark(&trace, format!("disposed {label}"));
            Ok(())
        });
        Ok(ExtensionComponent {
            component: input,
            dispose: Some(dispose),
            set_expanded: None,
            width: None,
        })
    })
}

/// The rendered first row of a retained widget component.
fn widget_row(host: &Screen, key: &str) -> String {
    let widgets = host.widgets.borrow();
    let (.., widget) = widgets
        .iter()
        .find(|(retained, ..)| retained == key)
        .unwrap();
    let Widget::Built(built) = widget else {
        panic!("widget {key} has lines");
    };
    built.component.render(30)[0].clone()
}

/// Sets line and factory widgets in each placement on the host's real writer.
fn widgets(ui: &Ui, host: &Screen, trace: &Trace) {
    let lines = Some(WidgetContent::Lines(vec!["a".into(), String::new()]));
    ui.set_widget("lines".into(), lines, None).unwrap();
    let below = Some(ExtensionWidgetOptions {
        placement: Some(WidgetPlacement::BelowEditor),
    });
    let built = Some(WidgetContent::Factory(input_factory(
        &host.api, trace, "built",
    )));
    ui.set_widget("built".into(), built, below).unwrap();
    assert_eq!(
        *trace.borrow(),
        ["built built", "returning built", "foreign built"]
    );
    let above = Some(ExtensionWidgetOptions {
        placement: Some(WidgetPlacement::AboveEditor),
    });
    let factory = Some(WidgetContent::Factory(input_factory(
        &host.api, trace, "above",
    )));
    ui.set_widget("above".into(), factory, above).unwrap();
    let placements: Vec<_> = host
        .widgets
        .borrow()
        .iter()
        .map(|(key, at, _)| (key.clone(), *at))
        .collect();
    assert_eq!(
        placements,
        [
            ("lines".to_owned(), WidgetPlacement::AboveEditor),
            ("built".to_owned(), WidgetPlacement::BelowEditor),
            ("above".to_owned(), WidgetPlacement::AboveEditor),
        ]
    );
    assert!(matches!(&host.widgets.borrow()[0].2, Widget::Lines(lines) if lines == &["a", ""]));
    let children = host.tui.children();
    let built: Vec<ComponentHandle> = host
        .widgets
        .borrow()
        .iter()
        .filter_map(|(.., widget)| match widget {
            Widget::Built(built) => Some(built.component.clone()),
            Widget::Lines(_) => None,
        })
        .collect();
    assert_eq!(children.borrow().len(), built.len());
    assert!(
        children
            .borrow()
            .iter()
            .zip(&built)
            .all(|(child, built)| Rc::ptr_eq(child, built))
    );
}

/// Uses a retained widget, then removes widgets and a header, disposing each once.
fn widget_input_and_removal(ui: &Ui, host: &Screen, trace: &Trace) {
    {
        let widgets = host.widgets.borrow();
        let Widget::Built(component) = &widgets[1].2 else {
            panic!("widget built has lines")
        };
        component
            .component
            .input_handler()
            .unwrap()
            .handle_input("!");
        component.component.invalidate();
    }
    let row = widget_row(host, "built");
    assert!(row.starts_with("> !\u{1b}[7mb\u{1b}[27muilt"), "{row:?}");
    trace.borrow_mut().clear();
    let below = Some(ExtensionWidgetOptions {
        placement: Some(WidgetPlacement::BelowEditor),
    });
    ui.set_widget("built".into(), None, below).unwrap();
    ui.set_widget("built".into(), None, None).unwrap();
    ui.set_header(Some(input_factory(&host.api, trace, "header")))
        .unwrap();
    ui.set_header(None).unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            "disposed built",
            "built header",
            "returning header",
            "foreign header",
            "disposed header",
        ]
    );
}

/// A successful theme selection.
const APPLIED: ThemeSetResult = ThemeSetResult {
    success: true,
    error: None,
};

/// The styled accent of the theme a view currently reads.
fn accent(theme: &LiveTheme) -> String {
    theme.get().unwrap().fg(&ThemeColor::Accent, "x").unwrap()
}

/// Follows publication through retained live views and keeps prepared instances fixed.
fn live_theme_follows_publication(ui: &Ui, host: &Screen, fallback: &LiveTheme) -> Rc<Theme> {
    let live = ui.theme();
    let unpublished = live.get().map(|_| ()).unwrap_err().to_string();
    assert_eq!(
        unpublished,
        "Theme not initialized. Call initTheme() first."
    );
    let first = published::theme(0);
    host.state.set_theme_instance(Rc::clone(&first)).unwrap();
    assert!(Rc::ptr_eq(&live.get().unwrap(), &first));
    let prepared = LiveTheme::from_theme(Rc::clone(&first));
    assert_eq!(accent(&live), first.fg(&ThemeColor::Accent, "x").unwrap());
    let set = ui.set_theme(ThemeSelection::Name("custom".into())).unwrap();
    assert_eq!(set, APPLIED);
    for view in [&live, fallback] {
        assert!(Rc::ptr_eq(&view.get().unwrap(), &host.prepared));
        assert_eq!(
            accent(view),
            host.prepared.fg(&ThemeColor::Accent, "x").unwrap()
        );
    }
    assert!(Rc::ptr_eq(&prepared.get().unwrap(), &first));
    assert_ne!(accent(&prepared), accent(&live));
    first
}

/// Named and instance selections, enumeration and named lookup reach the host.
fn theme_selection_routes(ui: &Ui, host: &Screen, first: &Rc<Theme>) {
    let live = ui.theme();
    let missing = ui
        .set_theme(ThemeSelection::Name("missing".into()))
        .unwrap();
    let error = Some("Theme \"missing\" not found".to_owned());
    assert_eq!(
        missing,
        ThemeSetResult {
            success: false,
            error
        }
    );
    assert!(Rc::ptr_eq(&live.get().unwrap(), &host.prepared));
    let prepared = LiveTheme::from_theme(Rc::clone(first));
    assert_eq!(
        ui.set_theme(ThemeSelection::Theme(prepared)).unwrap(),
        APPLIED
    );
    assert!(Rc::ptr_eq(&live.get().unwrap(), first));
    let unpublished = ui.set_theme(ThemeSelection::Theme(LiveTheme::default()));
    assert_eq!(
        unpublished.unwrap_err().to_string(),
        "Theme not initialized. Call initTheme() first."
    );
    let names: Vec<_> = ui
        .get_all_themes()
        .unwrap()
        .into_iter()
        .map(|info| (info.name, info.path))
        .collect();
    assert_eq!(
        names,
        [
            ("dark".to_owned(), None),
            ("custom".to_owned(), Some("/themes/custom.json".to_owned()))
        ]
    );
    assert!(Rc::ptr_eq(
        &ui.get_theme("custom".into()).unwrap().unwrap(),
        &host.prepared
    ));
    assert!(ui.get_theme("missing".into()).unwrap().is_none());
}

#[test]
fn ui_capabilities_keep_factories_options_and_live_theme() {
    let (tui, _terminal) = ui_toolkit::screen();
    let bus = LocalBus::new();
    let owner = loaded("/screen", bus.controller.for_extension());
    let trace = Trace::default();
    let foreign = bus.controller.for_extension();
    foreign.on(
        "built",
        Rc::new({
            let trace = trace.clone();
            move |data, _| {
                mark(&trace, format!("foreign {}", data.as_str().unwrap()));
                Ok(None)
            }
        }),
    );
    let state = published::unpublished();
    let runner = ExtensionRunner::<Local>::new(
        owner.owners.clone(),
        owner.runtime.clone(),
        local_support::session::runner_session("/work").0,
        Rc::new(|_| {}),
        state.theme(),
    );
    let fallback = runner.get_ui_context().theme();
    assert!(fallback.get().is_err());
    let host = Rc::new(Screen::new(tui, state, owner.api));
    runner.set_ui_context(Some(ExtensionUIContext::new(host.clone())));
    let ui = runner.get_ui_context();
    widgets(&ui, &host, &trace);
    widget_input_and_removal(&ui, &host, &trace);
    let first = live_theme_follows_publication(&ui, &host, &fallback);
    theme_selection_routes(&ui, &host, &first);
    ui.set_title("標題".into()).unwrap();
    ui.set_editor_text("x".into()).unwrap();
    ui.paste_to_editor("\nmulti\nline".into()).unwrap();
    assert_eq!(ui.get_editor_text().unwrap(), "x\nmulti\nline");
    assert_eq!(host.calls.borrow().last().unwrap(), "title 標題");
}

/// A host recording each control it consumes.
#[derive(Default)]
struct Controls {
    /// Received controls, in order.
    calls: Trace,
    /// The tool expansion state.
    expanded: Cell<bool>,
}
impl ExtensionUIContextPort for Controls {
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        mark(&self.calls, format!("notify {message:?} {kind:?}"));
        Ok(())
    }
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        mark(&self.calls, format!("status {key} {text:?}"));
        Ok(())
    }
    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        mark(&self.calls, format!("message {message:?}"));
        Ok(())
    }
    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        mark(&self.calls, format!("visible {visible}"));
        Ok(())
    }
    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        let options = options.map(|options| (options.frames, options.interval_ms));
        mark(&self.calls, format!("indicator {options:?}"));
        Ok(())
    }
    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        mark(&self.calls, format!("label {label:?}"));
        Ok(())
    }
    fn set_tools_expanded(&self, expanded: bool) -> Result<(), EventError> {
        mark(&self.calls, format!("expanded {expanded}"));
        self.expanded.set(expanded);
        Ok(())
    }
    fn get_tools_expanded(&self) -> Result<bool, EventError> {
        Ok(self.expanded.get())
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// Every working-indicator shape, one at a time.
fn indicators() -> Vec<Option<WorkingIndicatorOptions>> {
    let frames = |frames: &[&str]| Some(frames.iter().map(|frame| (*frame).to_owned()).collect());
    [None, Some(0.0), Some(-1.0), Some(12.5)]
        .into_iter()
        .map(|interval_ms| {
            Some(WorkingIndicatorOptions {
                frames: None,
                interval_ms,
            })
        })
        .chain([
            Some(WorkingIndicatorOptions {
                frames: frames(&[]),
                interval_ms: None,
            }),
            Some(WorkingIndicatorOptions {
                frames: frames(&["●"]),
                interval_ms: None,
            }),
            Some(WorkingIndicatorOptions {
                frames: frames(&["a", "b"]),
                interval_ms: Some(80.0),
            }),
            None,
        ])
        .collect()
}

#[test]
fn ui_control_resets_preserve_absence_and_empty_values() {
    let host = Rc::new(Controls::default());
    let ui = ExtensionUIContext::new(host.clone());
    for text in [None, Some(String::new()), Some("x".to_owned())] {
        ui.set_status("k".into(), text.clone()).unwrap();
        ui.set_working_message(text.clone()).unwrap();
        ui.set_hidden_thinking_label(text).unwrap();
    }
    for value in [true, false, true] {
        ui.set_working_visible(value).unwrap();
        ui.set_tools_expanded(value).unwrap();
        assert_eq!(ui.get_tools_expanded().unwrap(), value);
    }
    for options in indicators() {
        ui.set_working_indicator(options).unwrap();
    }
    ui.notify(String::new(), None).unwrap();
    ui.notify("n".into(), Some(NotificationType::Warning))
        .unwrap();
    assert_eq!(
        *host.calls.borrow(),
        [
            "status k None",
            "message None",
            "label None",
            "status k Some(\"\")",
            "message Some(\"\")",
            "label Some(\"\")",
            "status k Some(\"x\")",
            "message Some(\"x\")",
            "label Some(\"x\")",
            "visible true",
            "expanded true",
            "visible false",
            "expanded false",
            "visible true",
            "expanded true",
            "indicator Some((None, None))",
            "indicator Some((None, Some(0.0)))",
            "indicator Some((None, Some(-1.0)))",
            "indicator Some((None, Some(12.5)))",
            "indicator Some((Some([]), None))",
            "indicator Some((Some([\"●\"]), None))",
            "indicator Some((Some([\"a\", \"b\"]), Some(80.0)))",
            "indicator None",
            "notify \"\" None",
            "notify \"n\" Some(Warning)",
        ]
    );
}

/// A handler choosing its result from the raw input it records.
fn deciding_handler(api: &ExtensionAPI<Local>, seen: &Trace) -> TerminalInputHandler {
    let seen = seen.clone();
    api.ui_callback(move |data: String, _| {
        mark(&seen, format!("{data:?}"));
        Ok(match data.as_str() {
            "c" | "\u{1}" => InputListenerResult::Consume,
            "r" => InputListenerResult::Replace("R".into()),
            "e" => InputListenerResult::Replace(String::new()),
            "!" => return Err(Box::new(error_support::Marker(7_u64))),
            _ => InputListenerResult::Pass,
        })
    })
}

#[test]
fn terminal_input_keeps_consumption_replacement_and_unsubscribe() {
    let (tui, terminal) = ui_toolkit::screen();
    let editor = Rc::new(Input::new());
    tui.set_focus(Some(editor.clone()));
    tui.start().unwrap();
    let bus = LocalBus::new();
    let callbacks = loaded("/callbacks", bus.controller.for_extension());
    let host = Rc::new(Keyboard::new(tui));
    let ui = ExtensionUIContext::new(host.clone());
    let seen = Trace::default();
    let unsubscribe = ui
        .on_terminal_input(deciding_handler(&callbacks.api, &seen))
        .unwrap();
    for data in ["p", "c", "r", "e", "!", "é", "\u{1}"] {
        terminal.send_input(data);
    }
    assert_eq!(editor.get_value(), "pR!é");
    assert_eq!(
        *seen.borrow(),
        [
            "\"p\"",
            "\"c\"",
            "\"r\"",
            "\"e\"",
            "\"!\"",
            "\"é\"",
            "\"\\u{1}\""
        ]
    );
    let errors = host.errors.borrow();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0]
            .downcast_ref::<error_support::Marker<u64>>()
            .map(|m| m.0),
        Some(7)
    );
    let retained = Trace::default();
    let observer = retained.clone();
    let dropped = ui.on_terminal_input(callbacks.api.ui_callback(move |data: String, _| {
        mark(&observer, data);
        Ok(InputListenerResult::Pass)
    }));
    drop(dropped.unwrap());
    unsubscribe().unwrap();
    unsubscribe().unwrap();
    terminal.send_input("c");
    assert_eq!(editor.get_value(), "pR!éc");
    assert_eq!(seen.borrow().len(), 7);
    assert_eq!(*retained.borrow(), ["c"]);
}

/// A wrapper that tags the prefix and applied text of the provider it wraps.
struct Tagged {
    /// The wrapped current provider.
    inner: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    /// The tag this wrapper adds.
    tag: &'static str,
}
impl AutocompleteProvider for Tagged {
    type Signal = Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        Box::pin(async move {
            let mut suggestions = self.inner.get_suggestions(lines, cursor, options).await?;
            if let Some(suggestions) = &mut suggestions {
                suggestions.prefix.insert_str(0, self.tag);
            }
            Ok(suggestions)
        })
    }
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        let mut result = self.inner.apply_completion(lines, cursor, item, prefix);
        result.lines[0].insert_str(0, self.tag);
        result
    }
}

/// A host stacking extension wrappers on its current provider.
struct Completion {
    /// The current provider.
    current: RefCell<Rc<dyn AutocompleteProvider<Signal = Cancellation>>>,
}
impl ExtensionUIContextPort for Completion {
    fn add_autocomplete_provider(
        &self,
        factory: AutocompleteProviderFactory,
    ) -> Result<(), EventError> {
        let current = self.current.borrow().clone();
        let next = factory.call(current)?;
        drop(self.current.replace(next));
        Ok(())
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// The wrapped provider's suggestions, applied completion and cancellation.
async fn suggested(current: &Rc<dyn AutocompleteProvider<Signal = Cancellation>>) {
    let signal = Cancellation::new();
    assert_eq!(suggest(current, "", &signal).await, Ok(None));
    assert_eq!(
        suggest(current, "none", &signal).await,
        Ok(Some(("21none".into(), Vec::new())))
    );
    let ordered = Some(("21x".to_owned(), vec!["b".to_owned(), "a".to_owned()]));
    assert_eq!(suggest(current, "x", &signal).await, Ok(ordered));
    let item = AutocompleteItem {
        value: "b".into(),
        label: "b".into(),
        description: None,
    };
    let applied = current.apply_completion(
        &["x".into()],
        CursorPosition { line: 0, col: 1 },
        &item,
        "x",
    );
    assert_eq!(applied.lines, ["21x->b"]);
    signal.abort();
    assert_eq!(
        suggest(current, "x", &signal).await,
        Err("cancelled".into())
    );
}

/// Suggestions for one line from `provider`, with their prefix and values.
async fn suggest(
    provider: &Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    text: &str,
    signal: &Cancellation,
) -> Result<Option<(String, Vec<String>)>, String> {
    let lines = [text.to_owned()];
    let cursor = CursorPosition {
        line: 0,
        col: text.len(),
    };
    let options = CompletionOptions {
        signal,
        force: None,
    };
    let suggestions = provider.get_suggestions(&lines, cursor, options).await;
    suggestions
        .map(|found| {
            found.map(|found| {
                (
                    found.prefix,
                    found.items.into_iter().map(|item| item.value).collect(),
                )
            })
        })
        .map_err(|error| error.to_string())
}

#[test]
fn autocomplete_wrappers_keep_current_provider_and_errors() {
    local(async {
        let base: Rc<dyn AutocompleteProvider<Signal = Cancellation>> = Rc::new(Words);
        let bus = LocalBus::new();
        let callbacks = loaded("/callbacks", bus.controller.for_extension());
        let host = Rc::new(Completion {
            current: RefCell::new(base.clone()),
        });
        let ui = ExtensionUIContext::new(host.clone());
        let received = Rc::new(RefCell::new(Vec::new()));
        let wrap = |tag| -> AutocompleteProviderFactory {
            let received = received.clone();
            callbacks.api.ui_callback(
                move |current: Rc<dyn AutocompleteProvider<Signal = Cancellation>>, _| {
                    received.borrow_mut().push(current.clone());
                    let wrapped: Rc<dyn AutocompleteProvider<Signal = Cancellation>> =
                        Rc::new(Tagged {
                            inner: current,
                            tag,
                        });
                    Ok(wrapped)
                },
            )
        };
        ui.add_autocomplete_provider(wrap("1")).unwrap();
        let first = host.current.borrow().clone();
        ui.add_autocomplete_provider(wrap("2")).unwrap();
        assert!(
            Rc::ptr_eq(&received.borrow()[0], &base) && Rc::ptr_eq(&received.borrow()[1], &first)
        );
        let current = host.current.borrow().clone();
        let failing: AutocompleteProviderFactory = callbacks
            .api
            .ui_callback(|_, _| Err("wrapper failed".into()));
        assert_eq!(
            ui.add_autocomplete_provider(failing)
                .unwrap_err()
                .to_string(),
            "wrapper failed"
        );
        assert!(Rc::ptr_eq(&host.current.borrow(), &current));
        suggested(&current).await;
    });
}

/// A host retaining an editor factory and building it on request.
struct Editors {
    /// The real writer.
    tui: TUI,
    /// The application bindings factories receive.
    keys: KeybindingsManager,
    /// The facade editor factories are bound to.
    api: ExtensionAPI<Local>,
    /// The current custom editor factory.
    factory: RefCell<Option<EditorFactory>>,
}
impl Editors {
    /// A host over a fresh writer and the application bindings.
    fn installed(api: ExtensionAPI<Local>) -> Rc<Self> {
        Rc::new(Self {
            tui: ui_toolkit::screen().0,
            keys: application_keys(),
            api,
            factory: RefCell::default(),
        })
    }
    /// Builds the current factory's editor with the host's real operands.
    fn build(&self) -> Result<Option<Rc<dyn EditorComponent<Signal = Cancellation>>>, EventError> {
        let Some(factory) = self.factory.borrow().clone() else {
            return Ok(None);
        };
        let theme = get_editor_theme(LiveTheme::default());
        factory
            .call((self.tui.clone(), theme, factory_keys(self.keys.clone())))
            .map(|built| Some(built.editor()))
    }
}
impl ExtensionUIContextPort for Editors {
    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        drop(self.factory.replace(factory));
        Ok(())
    }
    fn get_editor_component(&self) -> Result<Option<EditorFactory>, EventError> {
        Ok(self.factory.borrow().clone())
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// Application bindings: the toolkit actions plus one application action.
fn application_keys() -> KeybindingsManager {
    let custom = KeybindingDefinition {
        default_keys: KeybindingKeys::Single("ctrl+g".into()),
        description: None,
    };
    let definitions = TUI_KEYBINDINGS
        .iter()
        .cloned()
        .chain([("app.custom".to_owned(), custom)])
        .collect();
    KeybindingsManager::new(definitions, Vec::new())
}

/// Shared application bindings with an independently supplied reload.
fn factory_keys(manager: KeybindingsManager) -> ExtensionKeybindings {
    let shared = manager.clone();
    ExtensionKeybindings {
        manager,
        reload: Some(Rc::new(move || {
            shared.set_user_bindings(vec![(
                "app.custom".into(),
                Some(KeybindingKeys::Single("ctrl+j".into())),
            )]);
        })),
    }
}

/// An editor factory that requires the application action and keeps its manager.
fn vim_factory(
    api: &ExtensionAPI<Local>,
    captured: &Rc<RefCell<Option<ExtensionKeybindings>>>,
) -> EditorFactory {
    let captured = captured.clone();
    api.ui_callback(
        move |(tui, theme, keys): (TUI, EditorTheme, ExtensionKeybindings), _| {
            if keys.get_keys("app.custom").is_empty() {
                return Err("toolkit defaults".into());
            }
            let editor: Rc<dyn EditorComponent<Signal = Cancellation>> =
                Rc::new(Editor::new(&tui, theme, EditorOptions::default()));
            *captured.borrow_mut() = Some(keys);
            Ok(EditorFactoryOutcome::Editor(editor))
        },
    )
}

#[test]
fn custom_editor_factory_keeps_theme_keys_and_live_bindings() {
    let bus = LocalBus::new();
    let host = Editors::installed(loaded("/editors", bus.controller.for_extension()).api);
    let (ui, tui) = (ExtensionUIContext::new(host.clone()), host.tui.clone());
    let captured = Rc::default();
    let factory = vim_factory(&host.api, &captured);
    ui.set_editor_component(Some(factory.clone())).unwrap();
    assert!(ui.get_editor_component().unwrap().unwrap().ptr_eq(&factory));
    let editor = host.build().unwrap().unwrap();
    editor.set_text("hello");
    assert_eq!(editor.get_text(), "hello");
    assert!(ui.get_editor_component().unwrap().unwrap().ptr_eq(&factory));
    let keys = captured.borrow().clone().unwrap();
    assert_eq!(keys.get_keys("app.custom"), ["ctrl+g"]);
    let rebound = vec![(
        "app.custom".to_owned(),
        Some(KeybindingKeys::Single("ctrl+h".into())),
    )];
    host.keys.set_user_bindings(rebound);
    assert_eq!(keys.get_keys("app.custom"), ["ctrl+h"]);
    let mut snapshot = keys.get_effective_config();
    snapshot.clear();
    keys.reload();
    assert_eq!(keys.get_keys("app.custom"), ["ctrl+j"]);
    assert!(!keys.get_effective_config().is_empty());
    let toolkit = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new());
    let theme = get_editor_theme(LiveTheme::default());
    let defaults = factory.call((tui, theme, factory_keys(toolkit)));
    assert_eq!(
        defaults.map(|_| ()).unwrap_err().to_string(),
        "toolkit defaults"
    );
    let failing: EditorFactory = host
        .api
        .ui_callback(|(_, _, _), _| Err("editor failed".into()));
    ui.set_editor_component(Some(failing)).unwrap();
    assert_eq!(
        host.build().map(|_| ()).unwrap_err().to_string(),
        "editor failed"
    );
    ui.set_editor_component(None).unwrap();
    assert!(ui.get_editor_component().unwrap().is_none());
    assert!(host.build().unwrap().is_none());
}

/// A host installing a custom footer over supplied metadata.
struct Footers {
    /// The real writer.
    tui: TUI,
    /// The metadata footers read.
    data: Rc<dyn ReadonlyFooterDataProvider>,
    /// The custom footer.
    footer: RefCell<Option<ExtensionComponent>>,
}
impl ExtensionUIContextPort for Footers {
    fn set_footer(&self, factory: Option<FooterFactory>) -> Result<(), EventError> {
        let footer = factory
            .map(|factory| {
                factory.call((
                    self.tui.clone(),
                    LiveTheme::default(),
                    Rc::clone(&self.data),
                ))
            })
            .transpose()?;
        let previous = self.footer.replace(footer);
        ui_toolkit::dispose(previous);
        Ok(())
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// What the branch footer retained from its factory call.
#[derive(Default)]
struct Kept {
    /// Removers returned by the metadata.
    removers: RefCell<Vec<Box<dyn Fn()>>>,
    /// The live status view.
    statuses: RefCell<Option<ExtensionStatuses>>,
}

/// A footer that subscribes one callback twice and emits from it through its owned bus.
fn branch_footer(api: &ExtensionAPI<Local>, trace: &Trace, kept: &Rc<Kept>) -> FooterFactory {
    let (trace, kept) = (trace.clone(), kept.clone());
    api.ui_callback(
        move |(_, _, data): (TUI, LiveTheme, Rc<dyn ReadonlyFooterDataProvider>), emitter| {
            let (branch, count) = (data.get_git_branch(), data.get_available_provider_count());
            mark(&trace, format!("branch {branch:?} providers {count}"));
            *kept.statuses.borrow_mut() = Some(data.get_extension_statuses());
            let (events, trace) = (emitter.events(), trace.clone());
            let callback: Rc<dyn Fn()> = Rc::new(move || {
                events.invoke_callback(|emitter| {
                    mark(&trace, "changed");
                    emitter.emit("branch", json!(null));
                    mark(&trace, "returning");
                });
            });
            kept.removers
                .borrow_mut()
                .push(data.on_branch_change(Rc::clone(&callback)));
            kept.removers
                .borrow_mut()
                .push(data.on_branch_change(callback));
            Ok(ExtensionComponent {
                component: Rc::new(Input::new()),
                dispose: None,
                set_expanded: None,
                width: None,
            })
        },
    )
}

/// The ordered pairs a status walk yields.
fn pairs(walk: impl IntoIterator<Item = (String, String)>) -> Vec<String> {
    walk.into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Edits statuses after the footer was built and reads them through its retained view.
fn statuses_stay_live(metadata: &Metadata, view: &ExtensionStatuses) {
    let statuses = &metadata.statuses;
    statuses.insert("a".into(), "1".into());
    statuses.insert("b".into(), "2".into());
    statuses.insert("a".into(), "3".into());
    assert_eq!(pairs(view.iter()), ["a=3", "b=2"]);
    let mut walk = view.iter();
    assert_eq!(walk.next(), Some(("a".into(), "3".into())));
    statuses.remove("b");
    statuses.insert("c".into(), "4".into());
    assert_eq!(pairs(&mut walk), ["c=4"]);
    statuses.insert("d".into(), "5".into());
    assert_eq!(walk.next(), None);
    statuses.remove("a");
    statuses.insert("a".into(), "6".into());
    assert_eq!(pairs(view), ["c=4", "d=5", "a=6"]);
    assert_eq!(
        (view.get("a").as_deref(), view.contains_key("b"), view.len()),
        (Some("6"), false, 3)
    );
    statuses.clear();
    assert!(view.is_empty());
}

#[test]
fn footer_factories_keep_live_metadata_and_subscription_identity() {
    let (tui, _terminal) = ui_toolkit::screen();
    let bus = LocalBus::new();
    let owner = loaded("/footer", bus.controller.for_extension());
    let trace = Trace::default();
    owner.api.events.on("branch", listener(&trace, "same"));
    bus.controller
        .for_extension()
        .on("branch", listener(&trace, "foreign"));
    let released = Rc::new(Cell::new(false));
    let metadata = Rc::new(Metadata::new(Rc::new(Released(released.clone()))));
    let host = Rc::new(Footers {
        tui,
        data: metadata.clone(),
        footer: RefCell::default(),
    });
    let ui = ExtensionUIContext::new(host.clone());
    let kept = Rc::new(Kept::default());
    ui.set_footer(Some(branch_footer(&owner.api, &trace, &kept)))
        .unwrap();
    assert_eq!(metadata.callbacks.len(), 1);
    for (_, callback) in &metadata.callbacks {
        callback();
    }
    assert_eq!(
        *trace.borrow(),
        [
            "branch Some(\"main\") providers 2.5",
            "changed",
            "same",
            "returning",
            "foreign"
        ]
    );
    statuses_stay_live(&metadata, kept.statuses.borrow().as_ref().unwrap());
    kept.removers.borrow()[1]();
    assert!(metadata.callbacks.is_empty());
    kept.removers.borrow()[0]();
    ui.set_footer(None).unwrap();
    drop((ui, host, metadata));
    assert!(!released.get());
    kept.removers.borrow_mut().clear();
    assert!(released.get());
}

/// A distinguishable original error.
#[derive(Debug, PartialEq)]
struct Marked(u8);
impl std::fmt::Display for Marked {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "marked {}", self.0)
    }
}
impl std::error::Error for Marked {}

/// A shown overlay's handle, shared by the host and the extension.
type SharedOverlay = Rc<RefCell<Box<dyn OverlayHandle>>>;

/// One admitted custom operation; the host owns its single close gate.
struct Session {
    /// The real writer the editor is restored on.
    tui: TUI,
    /// Host transcript.
    trace: Trace,
    /// Whether the operation closed.
    closed: Cell<bool>,
    /// The editor restoration failure, if restoring fails.
    restore_error: Option<&'static str>,
    /// Settles the caller's observation.
    settle: RefCell<Option<oneshot::Sender<Result<(), EventError>>>>,
    /// The installed component.
    component: RefCell<Option<ExtensionComponent>>,
    /// The shown overlay's shared handle.
    overlay: RefCell<Option<SharedOverlay>>,
}
impl Session {
    /// Settles the caller's observation once.
    fn finish(&self, result: Result<(), EventError>) {
        let settle = self.settle.borrow_mut().take();
        if let Some(settle) = settle {
            drop(settle.send(result));
        }
    }
    /// Closes once: hides the overlay or restores the editor, settles, then disposes.
    fn close(&self) -> Result<bool, EventError> {
        if self.closed.replace(true) {
            return Ok(false);
        }
        let overlay = self.overlay.borrow_mut().take();
        if let Some(overlay) = overlay {
            overlay.borrow_mut().hide()?;
        } else if let Some(message) = self.restore_error {
            self.finish(Err(message.into()));
            return Err(message.into());
        } else {
            self.tui.set_focus(None);
        }
        mark(&self.trace, "closed");
        self.finish(Ok(()));
        let component = self.component.borrow_mut().take();
        ui_toolkit::dispose(component);
        Ok(true)
    }
    /// Installs a built component unless the operation already closed.
    fn install(
        &self,
        tui: &TUI,
        component: ExtensionComponent,
        options: Option<&CustomUIOptions>,
    ) -> Result<(), EventError> {
        if self.closed.get() {
            mark(&self.trace, "not installed");
            return Ok(());
        }
        let shown = component.component.clone();
        *self.component.borrow_mut() = Some(component);
        let Some(options) = options.filter(|options| options.overlay == Some(true)) else {
            tui.set_focus(Some(shown));
            mark(&self.trace, "installed");
            return Ok(());
        };
        let resolved = match &options.overlay_options {
            Some(OverlayOptionsSource::Static(options)) => Some(options.clone()),
            Some(OverlayOptionsSource::Resolver(resolve)) => Some(resolve.call(())?),
            None => None,
        };
        let width = resolved
            .as_ref()
            .and_then(|options| options.borrow().min_width);
        let handle = Rc::new(RefCell::new(tui.show_overlay(shown, resolved)?));
        *self.overlay.borrow_mut() = Some(handle.clone());
        mark(&self.trace, format!("overlay {width:?}"));
        if let Some(on_handle) = &options.on_handle {
            on_handle.call(handle)?;
        }
        Ok(())
    }
}

/// A host showing custom components on a real writer.
struct Customs {
    /// The real writer.
    tui: TUI,
    /// The facade custom factories are bound to.
    api: ExtensionAPI<Local>,
    /// Host transcript.
    trace: Trace,
    /// The editor restoration failure for later operations.
    restore_error: Cell<Option<&'static str>>,
    /// Admitted operations, retained independently of their observers.
    sessions: RefCell<Vec<Rc<Session>>>,
    /// Pending asynchronous builds.
    builds: RefCell<Vec<tokio::task::JoinHandle<()>>>,
}
impl Customs {
    /// A host over a fresh writer whose factories are bound to `api`.
    fn new(api: ExtensionAPI<Local>) -> Self {
        Self {
            tui: ui_toolkit::screen().0,
            api,
            trace: Trace::default(),
            restore_error: Cell::new(None),
            sessions: RefCell::default(),
            builds: RefCell::default(),
        }
    }
    /// Waits for every pending asynchronous build.
    async fn settle(&self) {
        let builds = std::mem::take(&mut *self.builds.borrow_mut());
        for build in builds {
            build.await.unwrap();
        }
    }
    /// Continues an asynchronous build on the local task set.
    fn later(
        &self,
        session: Rc<Session>,
        pending: ExtensionUIFuture<ExtensionComponent>,
        options: Option<CustomUIOptions>,
    ) {
        let tui = self.tui.clone();
        self.builds
            .borrow_mut()
            .push(tokio::task::spawn_local(async move {
                let installed = pending
                    .await
                    .and_then(|component| session.install(&tui, component, options.as_ref()));
                if let Err((error, false)) =
                    installed.map_err(|error| (error, session.closed.get()))
                {
                    session.finish(Err(error));
                }
            }));
    }
}
impl ExtensionUIContextPort for Customs {
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (settle, settled) = oneshot::channel();
        let session = Rc::new(Session {
            tui: self.tui.clone(),
            trace: self.trace.clone(),
            closed: Cell::new(false),
            restore_error: self.restore_error.get(),
            settle: RefCell::new(Some(settle)),
            component: RefCell::default(),
            overlay: RefCell::default(),
        });
        self.sessions.borrow_mut().push(session.clone());
        let close: CustomClose = Rc::new({
            let session = session.clone();
            move || session.close()
        });
        let keys = factory_keys(application_keys());
        let built = factory.call((self.tui.clone(), LiveTheme::default(), keys, close));
        let installed = match built {
            Ok(CustomFactoryOutcome::Ready(component)) => {
                session.install(&self.tui, component, options.as_ref())
            }
            Ok(CustomFactoryOutcome::Async(pending)) => {
                self.later(session.clone(), pending, options);
                Ok(())
            }
            Err(error) => Err(error),
        };
        if let Err((error, false)) = installed.map_err(|error| (error, session.closed.get())) {
            session.finish(Err(error));
        }
        Box::pin(async move { settled.await.unwrap_or(Ok(())) })
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

/// A slot for the `done` a factory received.
type DoneSlot<T> = Rc<RefCell<Option<CustomDone<T>>>>;

/// A component disposing into the host transcript.
fn traced_component(
    api: &ExtensionAPI<Local>,
    trace: &Trace,
    input: Rc<Input>,
) -> ExtensionComponent {
    let trace = trace.clone();
    ExtensionComponent {
        component: input,
        dispose: Some(api.ui_callback(move |(), _| {
            mark(&trace, "disposed");
            Ok(())
        })),
        set_expanded: None,
        width: None,
    }
}

/// A factory keeping `done` and returning a ready input component.
fn ready_factory<T: 'static>(
    api: &ExtensionAPI<Local>,
    trace: &Trace,
    input: &Rc<Input>,
    slot: &DoneSlot<T>,
) -> CustomFactory<T> {
    let (bind, inner, trace, input, slot) = (
        api.clone(),
        api.clone(),
        trace.clone(),
        input.clone(),
        slot.clone(),
    );
    bind.ui_callback(
        move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, CustomDone<T>), _| {
            *slot.borrow_mut() = Some(done);
            Ok(CustomFactoryOutcome::Ready(traced_component(
                &inner,
                &trace,
                input.clone(),
            )))
        },
    )
}

/// Calls the retained `done` with `value`.
fn complete<T>(slot: &DoneSlot<T>, value: T) -> Result<(), EventError> {
    let done = slot.borrow().clone().unwrap();
    done(value)
}

/// Ready and suspended builders both keep the actual returned local component.
async fn both_factory_outcomes(ui: &Ui, host: &Customs) {
    let (input, slot) = (Rc::new(Input::new()), DoneSlot::default());
    let observed = ui.custom(ready_factory(&host.api, &host.trace, &input, &slot), None);
    complete(&slot, input.clone()).unwrap();
    complete(&slot, Rc::new(Input::new())).unwrap();
    assert!(Rc::ptr_eq(&observed.await.unwrap().unwrap(), &input));
    let (gate, opened) = oneshot::channel::<()>();
    let opened = RefCell::new(Some(opened));
    let (trace, pending_input, slot) = (
        host.trace.clone(),
        Rc::new(Input::new()),
        DoneSlot::default(),
    );
    let kept = slot.clone();
    let suspended = pending_input.clone();
    let trace_api = host.api.clone();
    let factory: CustomFactory<Rc<Input>> = host.api.ui_callback(move |(_, _, _, done), _| {
        *kept.borrow_mut() = Some(done);
        let (opened, input, trace) = (
            opened.borrow_mut().take().unwrap(),
            suspended.clone(),
            trace.clone(),
        );
        let api = trace_api.clone();
        Ok(CustomFactoryOutcome::Async(Box::pin(async move {
            opened.await.unwrap();
            Ok(traced_component(&api, &trace, input))
        })))
    });
    let observed = ui.custom(factory, None);
    tokio::task::yield_now().await;
    assert_eq!(*host.trace.borrow(), ["installed", "closed", "disposed"]);
    gate.send(()).unwrap();
    host.settle().await;
    complete(&slot, pending_input.clone()).unwrap();
    let result = observed.await.unwrap().unwrap();
    pending_input.set_value("edited".into());
    assert!(Rc::ptr_eq(&result, &pending_input) && result.get_value() == "edited");
    assert_eq!(
        *host.trace.borrow(),
        [
            "installed",
            "closed",
            "disposed",
            "installed",
            "closed",
            "disposed"
        ]
    );
}

/// Completes immediately from inside the factory with `value`.
async fn completed<T: 'static>(api: &ExtensionAPI<Local>, ui: &Ui, value: T) -> Option<T> {
    let value = RefCell::new(Some(value));
    let factory: CustomFactory<T> = api.ui_callback(
        move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, CustomDone<T>), _| {
            done(value.borrow_mut().take().unwrap())?;
            Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                component: Rc::new(Input::new()),
                dispose: None,
                set_expanded: None,
                width: None,
            }))
        },
    );
    ui.custom(factory, None).await.unwrap()
}

/// Native results keep their exact values without serialization.
async fn result_values(api: &ExtensionAPI<Local>, ui: &Ui) {
    assert_eq!(completed::<Option<u8>>(api, ui, None).await, Some(None));
    assert_eq!(completed(api, ui, ()).await, Some(()));
    assert_eq!(completed(api, ui, false).await, Some(false));
    assert_eq!(completed(api, ui, String::new()).await, Some(String::new()));
    assert_eq!(completed(api, ui, 0_u8).await, Some(0));
    assert_eq!(
        completed(api, ui, vec![("k", 1)]).await,
        Some(vec![("k", 1)])
    );
    let function: Rc<dyn Fn() -> u8> = Rc::new(|| 7);
    let returned = completed(api, ui, function.clone()).await.unwrap();
    assert!(Rc::ptr_eq(&returned, &function) && returned() == 7);
}

/// A factory failing with `error` after optionally completing with `value`.
fn failing_factory(api: &ExtensionAPI<Local>, value: Option<u8>, error: u8) -> CustomFactory<u8> {
    api.ui_callback(
        move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, CustomDone<u8>), _| {
            if let Some(value) = value {
                done(value)?;
            }
            Err(Box::new(Marked(error)))
        },
    )
}

/// Failures before commit propagate unchanged; committed values win; nothing hangs.
async fn failures_and_restoration(ui: &Ui, host: &Customs) {
    let failure = ui
        .custom(failing_factory(&host.api, None, 1), None)
        .await
        .unwrap_err();
    assert_eq!(failure.downcast_ref::<Marked>(), Some(&Marked(1)));
    assert_eq!(
        ui.custom(failing_factory(&host.api, Some(5), 2), None)
            .await
            .unwrap(),
        Some(5)
    );
    let late: CustomFactory<u8> = host.api.ui_callback(
        |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, CustomDone<u8>), _| {
            done(6)?;
            Ok(CustomFactoryOutcome::Async(Box::pin(async {
                Err(Box::new(Marked(3)) as EventError)
            })))
        },
    );
    let observed = ui.custom(late, None);
    host.settle().await;
    assert_eq!(observed.await.unwrap(), Some(6));
    host.restore_error.set(Some("restore failed"));
    let failure = ui
        .custom(failing_factory(&host.api, Some(1), 4), None)
        .await
        .unwrap_err();
    assert_eq!(failure.to_string(), "restore failed");
    let (input, slot) = (Rc::new(Input::new()), DoneSlot::<u8>::default());
    let observed = ui.custom(ready_factory(&host.api, &host.trace, &input, &slot), None);
    assert_eq!(
        complete(&slot, 2).unwrap_err().to_string(),
        "restore failed"
    );
    assert_eq!(observed.await.unwrap_err().to_string(), "restore failed");
    host.restore_error.set(None);
}

/// An overlay handle callback that hides and then refocuses the shown overlay.
fn refocusing(
    api: &ExtensionAPI<Local>,
    seen: &Rc<RefCell<Vec<SharedOverlay>>>,
) -> OverlayHandleCallback {
    let seen = seen.clone();
    api.ui_callback(move |handle: SharedOverlay, _| {
        handle.borrow_mut().set_hidden(true);
        assert!(handle.borrow().is_hidden());
        handle.borrow_mut().set_hidden(false);
        handle.borrow_mut().focus();
        seen.borrow_mut().push(handle);
        Ok(())
    })
}

/// Absent, disabled, static, resolved and default overlay options, in that order.
fn overlay_sources(
    api: &ExtensionAPI<Local>,
    seen: &Rc<RefCell<Vec<SharedOverlay>>>,
    width: &Rc<Cell<isize>>,
) -> [Option<CustomUIOptions>; 6] {
    let resolver: OverlayOptionsResolver = {
        let width = width.clone();
        api.ui_callback(move |(), _| {
            Ok(Rc::new(RefCell::new(OverlayOptions {
                min_width: Some(width.get()),
                ..OverlayOptions::default()
            })))
        })
    };
    let fixed = Rc::new(RefCell::new(OverlayOptions {
        min_width: Some(3),
        ..OverlayOptions::default()
    }));
    [
        None,
        Some(CustomUIOptions {
            overlay: Some(false),
            ..CustomUIOptions::default()
        }),
        Some(CustomUIOptions {
            overlay: Some(true),
            overlay_options: Some(OverlayOptionsSource::Static(fixed)),
            on_handle: Some(refocusing(api, seen)),
        }),
        Some(CustomUIOptions {
            overlay: Some(true),
            overlay_options: Some(OverlayOptionsSource::Resolver(resolver.clone())),
            on_handle: None,
        }),
        Some(CustomUIOptions {
            overlay: Some(true),
            overlay_options: Some(OverlayOptionsSource::Resolver(resolver)),
            on_handle: None,
        }),
        Some(CustomUIOptions {
            overlay: Some(true),
            ..CustomUIOptions::default()
        }),
    ]
}

/// Overlay flags, static and resolved options and the shared handle reach the host.
async fn overlays(ui: &Ui, host: &Customs) {
    let seen = Rc::default();
    let width = Rc::new(Cell::new(7));
    host.trace.borrow_mut().clear();
    for (index, options) in overlay_sources(&host.api, &seen, &width)
        .into_iter()
        .enumerate()
    {
        let (input, slot) = (Rc::new(Input::new()), DoneSlot::<usize>::default());
        let observed = ui.custom(
            ready_factory(&host.api, &host.trace, &input, &slot),
            options,
        );
        if index == 2 {
            let shown = host
                .sessions
                .borrow()
                .last()
                .unwrap()
                .overlay
                .borrow()
                .clone()
                .unwrap();
            assert!(Rc::ptr_eq(&seen.borrow()[0], &shown));
            assert!(!shown.borrow().is_hidden() && shown.borrow().is_focused());
        }
        if index == 3 {
            width.set(9);
        }
        complete(&slot, index).unwrap();
        assert_eq!(observed.await.unwrap(), Some(index));
    }
    assert!(!host.tui.has_overlay());
    let shown: Vec<_> = host
        .trace
        .borrow()
        .iter()
        .filter(|line| *line != "closed" && *line != "disposed")
        .cloned()
        .collect();
    assert_eq!(
        shown,
        [
            "installed",
            "installed",
            "overlay Some(3)",
            "overlay Some(7)",
            "overlay Some(9)",
            "overlay None"
        ]
    );
}

/// The host keeps admitted work after its observer is dropped; early completion blocks installation.
async fn admitted_work_outlives_its_observer(ui: &Ui, host: &Customs) {
    host.trace.borrow_mut().clear();
    let (input, slot) = (Rc::new(Input::new()), DoneSlot::<u8>::default());
    drop(ui.custom(ready_factory(&host.api, &host.trace, &input, &slot), None));
    let component = Rc::downgrade(&input);
    drop(input);
    assert!(component.upgrade().is_some());
    complete(&slot, 1).unwrap();
    assert!(component.upgrade().is_none());
    let (gate, opened) = oneshot::channel::<()>();
    let opened = RefCell::new(Some(opened));
    let trace = host.trace.clone();
    let api = host.api.clone();
    let early: CustomFactory<u8> = host.api.ui_callback(
        move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, CustomDone<u8>), _| {
            done(8)?;
            let (opened, trace) = (opened.borrow_mut().take().unwrap(), trace.clone());
            let api = api.clone();
            Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                opened.await.unwrap();
                Ok(traced_component(&api, &trace, Rc::new(Input::new())))
            })))
        },
    );
    assert_eq!(ui.custom(early, None).await.unwrap(), Some(8));
    gate.send(()).unwrap();
    host.settle().await;
    assert_eq!(
        *host.trace.borrow(),
        ["installed", "closed", "disposed", "closed", "not installed"]
    );
}

/// Typed completion keeps the same application manager the custom factory received.
async fn custom_factory_keys(api: &ExtensionAPI<Local>, ui: &Ui, trace: &Trace) {
    let (api, trace) = (api.clone(), trace.clone());
    let factory: CustomFactory<ExtensionKeybindings> = api.clone().ui_callback(
        move |(_, _, keys, done): (
            TUI,
            LiveTheme,
            ExtensionKeybindings,
            CustomDone<ExtensionKeybindings>,
        ),
              _| {
            done(keys)?;
            Ok(CustomFactoryOutcome::Ready(traced_component(
                &api,
                &trace,
                Rc::new(Input::new()),
            )))
        },
    );
    let keys = ui.custom(factory, None).await.unwrap().unwrap();
    assert_eq!(keys.get_keys("app.custom"), ["ctrl+g"]);
    let snapshot = keys.get_effective_config();
    keys.reload();
    assert_eq!(keys.get_keys("app.custom"), ["ctrl+j"]);
    assert_ne!(keys.get_effective_config(), snapshot);
}

#[test]
fn custom_ui_keeps_both_factory_outcomes_and_overlay_callbacks() {
    local(async {
        let bus = LocalBus::new();
        let callbacks = loaded("/callbacks", bus.controller.for_extension());
        let host = Rc::new(Customs::new(callbacks.api.clone()));
        let ui = ExtensionUIContext::new(host.clone());
        both_factory_outcomes(&ui, &host).await;
        result_values(&host.api, &ui).await;
        failures_and_restoration(&ui, &host).await;
        overlays(&ui, &host).await;
        admitted_work_outlives_its_observer(&ui, &host).await;
        custom_factory_keys(&host.api, &ui, &host.trace).await;
        bus.settle().await;
        assert!(bus.errors.borrow().is_empty());
        assert_eq!(bus.spawned.get(), 0);
    });
}

/// A renderer recording its label, custom type and expansion.
fn renderer<T: 'static>(trace: &Trace, label: &'static str) -> MessageRenderer<T, Local> {
    let trace = trace.clone();
    Rc::new(
        move |message: Rc<CustomMessage<T>>,
              options: MessageRenderOptions,
              _: LiveTheme,
              _: CallbackEmitter<'_, Local>| {
            mark(
                &trace,
                format!("{label} {} {}", message.custom_type, options.expanded),
            );
            Ok(None)
        },
    )
}

/// A custom message with typed details.
fn message<T>(custom_type: &str, details: Presence<T>) -> CustomMessage<T> {
    CustomMessage {
        custom_type: custom_type.into(),
        content: UserContent::Text("body".into()),
        display: true,
        details,
        timestamp: 1.5,
    }
}

/// Renders through the extension callback boundary.
fn render<T>(
    bus: &LocalBus,
    renderer: &MessageRenderer<T, Local>,
    message: Rc<CustomMessage<T>>,
    expanded: bool,
    theme: LiveTheme,
) -> Result<Option<ComponentHandle>, EventError> {
    bus.controller.invoke_callback(|emitter| {
        renderer(message, MessageRenderOptions { expanded }, theme, emitter)
    })
}

/// The label the current renderer for `custom_type` records, if one is selected.
fn rendered_by(
    runner: &ExtensionRunner<Local>,
    bus: &LocalBus,
    trace: &Trace,
    custom_type: &str,
) -> Option<String> {
    let renderer = runner.get_message_renderer::<Value>(custom_type)?;
    render(
        bus,
        &renderer,
        Rc::new(message(custom_type, Presence::Missing)),
        false,
        LiveTheme::default(),
    )
    .unwrap();
    trace.borrow_mut().pop()
}

#[test]
fn message_renderer_lookup_keeps_the_first_owner() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let (a, b) = (
        loaded("/a", bus.controller.for_extension()),
        loaded("/b", bus.controller.for_extension()),
    );
    let owners: ExtensionCollection<Local> =
        Rc::new(RwLock::new(vec![a.owner.clone(), b.owner.clone()]));
    let runner = runner(&owners);
    for (owner, label) in [(&a, "a"), (&b, "b")] {
        owner
            .api
            .register_message_renderer("note".into(), renderer::<Value>(&trace, label))
            .unwrap();
    }
    b.api
        .register_message_renderer("ノート\u{feff}".into(), renderer::<Value>(&trace, "b"))
        .unwrap();
    a.api
        .register_message_renderer("typed".into(), renderer::<u8>(&trace, "a"))
        .unwrap();
    b.api
        .register_message_renderer("typed".into(), renderer::<String>(&trace, "b"))
        .unwrap();
    assert_eq!(
        rendered_by(&runner, &bus, &trace, "note").as_deref(),
        Some("a note false")
    );
    assert_eq!(
        rendered_by(&runner, &bus, &trace, "ノート\u{feff}").as_deref(),
        Some("b ノート\u{feff} false")
    );
    for missing in ["", "missing", " note", "ノート", "Note"] {
        assert!(runner.get_message_renderer::<Value>(missing).is_none());
    }
    assert!(runner.get_message_renderer::<String>("typed").is_none());
    assert!(runner.get_message_renderer::<u8>("typed").is_some());
    a.api
        .register_message_renderer("late".into(), renderer::<Value>(&trace, "a"))
        .unwrap();
    assert_eq!(
        rendered_by(&runner, &bus, &trace, "late").as_deref(),
        Some("a late false")
    );
    collection_changes(&runner, &owners, &bus, &trace);
}

/// Reordering, removing and appending owners changes the selected owner.
fn collection_changes(
    runner: &ExtensionRunner<Local>,
    owners: &ExtensionCollection<Local>,
    bus: &LocalBus,
    trace: &Trace,
) {
    owners.write().unwrap().reverse();
    assert_eq!(
        rendered_by(runner, bus, trace, "note").as_deref(),
        Some("b note false")
    );
    assert!(runner.get_message_renderer::<String>("typed").is_some());
    owners.write().unwrap().remove(0);
    assert!(
        runner
            .get_message_renderer::<Value>("ノート\u{feff}")
            .is_none()
    );
    let c = loaded("/c", bus.controller.for_extension());
    c.api
        .register_message_renderer("appended".into(), renderer::<Value>(trace, "c"))
        .unwrap();
    owners.write().unwrap().push(c.owner);
    assert_eq!(
        rendered_by(runner, bus, trace, "appended").as_deref(),
        Some("c appended false")
    );
    assert!(trace.borrow().is_empty());
}

#[test]
fn message_renderer_replacement_preserves_slot_and_old_callback() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let owner = loaded("/replace", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    let released = Rc::new(Cell::new(false));
    let capture = Released(released.clone());
    let first_trace = trace.clone();
    let first: MessageRenderer<Value, Local> = Rc::new(
        move |message: Rc<CustomMessage>, _, _, _: CallbackEmitter<'_, Local>| {
            let _ = &capture;
            mark(&first_trace, format!("A {}", message.custom_type));
            Ok(None)
        },
    );
    owner
        .api
        .register_message_renderer("z".into(), first)
        .unwrap();
    owner
        .api
        .register_message_renderer("a".into(), renderer::<Value>(&trace, "a"))
        .unwrap();
    let retained = runner.get_message_renderer::<Value>("z").unwrap();
    owner
        .api
        .register_message_renderer("z".into(), renderer::<Value>(&trace, "B"))
        .unwrap();
    let keys: Vec<_> = owner
        .owner
        .read()
        .unwrap()
        .message_renderers
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["z", "a"]);
    assert_eq!(
        rendered_by(&runner, &bus, &trace, "z").as_deref(),
        Some("B z false")
    );
    owner.owners.write().unwrap().clear();
    drop(owner);
    let message = Rc::new(message("z", Presence::Missing));
    render(&bus, &retained, message, true, LiveTheme::default()).unwrap();
    assert_eq!(*trace.borrow(), ["A z"]);
    let alias = retained.clone();
    drop(retained);
    assert!(!released.get());
    drop(alias);
    assert!(released.get());
}

/// Details with a local terminal object, retained without copying.
struct Notes {
    /// Display text.
    text: String,
    /// A local component the renderer returns.
    input: Rc<Input>,
}

/// What the notes renderer retained from its calls.
#[derive(Default)]
struct Rendered {
    /// Received messages, in order.
    messages: RefCell<Vec<Rc<CustomMessage<Notes>>>>,
    /// Received themes, in order.
    themes: RefCell<Vec<LiveTheme>>,
    /// Received expansion flags.
    expanded: RefCell<Vec<bool>>,
}

/// A notes message whose details hold `input`.
fn notes_message(text: &str, input: &Rc<Input>) -> Rc<CustomMessage<Notes>> {
    let notes = Notes {
        text: text.into(),
        input: input.clone(),
    };
    Rc::new(message("notes", Presence::Present(notes)))
}

/// A renderer returning its details' local component and retaining its operands.
fn notes_renderer(rendered: &Rc<Rendered>) -> MessageRenderer<Notes, Local> {
    let rendered = rendered.clone();
    Rc::new(
        move |message: Rc<CustomMessage<Notes>>,
              options: MessageRenderOptions,
              theme: LiveTheme,
              _: CallbackEmitter<'_, Local>| {
            rendered.expanded.borrow_mut().push(options.expanded);
            rendered.themes.borrow_mut().push(theme);
            let component = match &message.details {
                Presence::Present(notes) if notes.text == "fail" => {
                    return Err(Box::new(Marked(9)));
                }
                Presence::Present(notes) => Some(notes.input.clone() as ComponentHandle),
                Presence::Missing | Presence::Null => None,
            };
            rendered.messages.borrow_mut().push(message);
            Ok(component)
        },
    )
}

#[test]
fn message_renderers_keep_messages_options_theme_and_errors() {
    let bus = LocalBus::new();
    let owner = loaded("/notes", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    let rendered = Rc::new(Rendered::default());
    owner
        .api
        .register_message_renderer("notes".into(), notes_renderer(&rendered))
        .unwrap();
    owner
        .api
        .register_message_renderer("count".into(), renderer::<u32>(&Trace::default(), "count"))
        .unwrap();
    let notes = runner.get_message_renderer::<Notes>("notes").unwrap();
    assert!(runner.get_message_renderer::<u32>("count").is_some());
    let input = Rc::new(Input::new());
    let shown = notes_message("shown", &input);
    let (state, live) = published::published(&published::theme(0));
    let returned = render(&bus, &notes, shown.clone(), true, live.clone())
        .unwrap()
        .unwrap();
    assert!(Rc::ptr_eq(&returned, &(input.clone() as ComponentHandle)));
    let prepared = published::theme(30);
    let missing = Rc::new(message("notes", Presence::Missing));
    assert!(
        render(
            &bus,
            &notes,
            missing,
            false,
            LiveTheme::from_theme(prepared.clone())
        )
        .unwrap()
        .is_none()
    );
    let failing = notes_message("fail", &input);
    let error = render(&bus, &notes, failing, false, live)
        .map(|_| ())
        .unwrap_err();
    assert_eq!(error.downcast_ref::<Marked>(), Some(&Marked(9)));
    assert_eq!(*rendered.expanded.borrow(), [true, false, false]);
    retained_operands(&rendered, &shown, &state, &prepared);
}

/// The renderer retained the shared message and live theme operands it received.
fn retained_operands(
    rendered: &Rendered,
    shown: &Rc<CustomMessage<Notes>>,
    state: &ThemeState,
    prepared: &Rc<Theme>,
) {
    let first = rendered.messages.borrow()[0].clone();
    assert!(Rc::ptr_eq(&first, shown));
    let CustomMessage {
        custom_type,
        content,
        display,
        timestamp,
        ..
    } = &*first;
    assert_eq!(
        (custom_type.as_str(), display, timestamp),
        ("notes", &true, &1.5)
    );
    assert!(matches!(content, UserContent::Text(text) if text == "body"));
    let next = published::theme(10);
    state.set_theme_instance(next.clone()).unwrap();
    let themes = rendered.themes.borrow();
    assert!(Rc::ptr_eq(&themes[0].get().unwrap(), &next));
    assert!(Rc::ptr_eq(&themes[1].get().unwrap(), prepared));
}

#[test]
fn message_renderer_registration_uses_existing_activity_guard() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    for (message, expected) in [
        (Some("custom stale"), "custom stale"),
        (None, STALE),
        (Some(""), ""),
    ] {
        let owner = loaded("/stale", bus.controller.for_extension());
        owner
            .api
            .register_message_renderer("kept".into(), renderer::<Value>(&trace, "kept"))
            .unwrap();
        owner.runtime.invalidate(message.map(Into::into));
        owner.runtime.invalidate(Some("later".into()));
        let error = owner
            .api
            .register_message_renderer("added".into(), renderer::<Value>(&trace, "added"));
        assert_eq!(error.unwrap_err().to_string(), expected);
        let keys: Vec<_> = owner
            .owner
            .read()
            .unwrap()
            .message_renderers
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["kept"]);
        assert!(
            runner(&owner.owners)
                .get_message_renderer::<Value>("kept")
                .is_some()
        );
    }
    let released = loaded("/released", bus.controller.for_extension());
    let Loaded {
        owner, api, owners, ..
    } = released;
    drop((owner, owners));
    let error = api.register_message_renderer("added".into(), renderer::<Value>(&trace, "added"));
    assert_eq!(error.unwrap_err().to_string(), STALE);
    let sibling = loaded("/sibling", bus.controller.for_extension());
    sibling
        .api
        .register_message_renderer("added".into(), renderer::<Value>(&trace, "sibling"))
        .unwrap();
    assert_eq!(
        rendered_by(&runner(&sibling.owners), &bus, &trace, "added").as_deref(),
        Some("sibling added false")
    );
}

/// A renderer using a captured facade, emitting to other extensions before returning.
fn facade_renderer(
    api: &ExtensionAPI<Local>,
    trace: &Trace,
    released: Released,
) -> MessageRenderer<Value, Local> {
    let (api, trace) = (api.clone(), trace.clone());
    Rc::new(move |_, _, _, emitter: CallbackEmitter<'_, Local>| {
        let _ = &released;
        mark(&trace, "render");
        emitter.emit("rendered", json!(null));
        mark(&trace, "returning");
        api.get_flag("verbose")?;
        Ok(None)
    })
}

/// An editor factory using a captured facade.
fn facade_editor(bind: &ExtensionAPI<Local>, released: Released) -> EditorFactory {
    let api = bind.clone();
    bind.ui_callback(
        move |(_, _, _): (TUI, EditorTheme, ExtensionKeybindings), _| {
            let _ = &released;
            api.get_flag("verbose")?;
            Err("unreachable".into())
        },
    )
}

#[test]
fn retained_ui_callbacks_release_captured_api_owners() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    bus.controller
        .for_extension()
        .on("rendered", listener(&trace, "foreign"));
    let owner = loaded("/captured", bus.controller.for_extension());
    let (renderer_released, editor_released) =
        (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let capturing = facade_renderer(&owner.api, &trace, Released(renderer_released.clone()));
    owner
        .api
        .register_message_renderer("captured".into(), capturing)
        .unwrap();
    let retained = runner(&owner.owners)
        .get_message_renderer::<Value>("captured")
        .unwrap();
    let message = Rc::new(message("captured", Presence::Missing));
    let rendered = render(
        &bus,
        &retained,
        message.clone(),
        false,
        LiveTheme::default(),
    );
    assert!(rendered.unwrap().is_none());
    assert_eq!(*trace.borrow(), ["render", "returning", "foreign"]);
    let host = Editors::installed(owner.api.clone());
    let ui = ExtensionUIContext::new(host.clone());
    ui.set_editor_component(Some(facade_editor(
        &owner.api,
        Released(editor_released.clone()),
    )))
    .unwrap();
    let Loaded {
        owner: strong,
        api,
        owners,
        ..
    } = owner;
    let weak = Rc::downgrade(&strong);
    drop((strong, owners));
    assert!(weak.upgrade().is_none());
    assert_eq!(
        api.register_message_renderer("late".into(), renderer::<Value>(&trace, "late"))
            .unwrap_err()
            .to_string(),
        STALE
    );
    let stale = render(&bus, &retained, message, false, LiveTheme::default()).map(|_| ());
    assert_eq!(stale.unwrap_err().to_string(), STALE);
    assert_eq!(host.build().map(|_| ()).unwrap_err().to_string(), STALE);
    assert!(!renderer_released.get() && !editor_released.get());
    drop(retained);
    assert!(renderer_released.get());
    ui.set_editor_component(None).unwrap();
    assert!(editor_released.get());
}

#[test]
fn ui_defaults_preserve_threaded_registration_contracts() {
    use maestro_extensions::types::MessageRenderer as DefaultRenderer;
    use maestro_extensions::{Extension, ExtensionAPI as DefaultApi, create_event_bus};
    let owner = Arc::new(RwLock::new(Extension::<maestro_models::Threaded>::new(
        "/default".into(),
        "/resolved".into(),
        local_support::source("/default"),
    )));
    let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
    let runtime = maestro_extensions::create_extension_runtime::<maestro_models::Threaded>();
    let bus = create_event_bus(Arc::new(|tail| drop(tokio::spawn(tail))), Arc::new(|_| {}));
    let api = DefaultApi::new(&owner, runtime.clone(), bus.for_extension(), "/work".into());
    let renderer: DefaultRenderer<u32> = Arc::new(
        |message: Arc<CustomMessage<u32>>, options, theme: LiveTheme, _| {
            let details = matches!(message.details, Presence::Present(7));
            Ok((details && options.expanded && theme.get().is_err())
                .then(|| Rc::new(Input::new()) as ComponentHandle))
        },
    );
    api.register_message_renderer("n".into(), renderer).unwrap();
    let runner = ExtensionRunner::<maestro_models::Threaded>::new(
        owners,
        runtime,
        local_support::session::runner_session("/work").0,
        Arc::new(|_| {}),
        LiveTheme::default(),
    );
    assert!(runner.get_message_renderer::<Value>("n").is_none());
    let renderer = runner.get_message_renderer::<u32>("n").unwrap();
    let elsewhere = std::thread::spawn(move || {
        let message = Arc::new(message("n", Presence::Present(7)));
        let options = MessageRenderOptions { expanded: true };
        bus.invoke_callback(|emitter| renderer(message, options, LiveTheme::default(), emitter))
            .unwrap()
            .is_some()
    });
    assert!(elsewhere.join().unwrap());
}

#[test]
fn retained_ui_errors_share_live_runner_listeners_and_original_extension_attribution() {
    let bus = LocalBus::new();
    let loaded = loaded("/original-extension", bus.controller.for_extension());
    let runner = runner(&loaded.owners);
    let host = ExtensionUIContext::new(Rc::new(limited_host::Limited));
    runner.set_ui_context_binding(Some(ExtensionUIContextBinding::borrowed(&host)));
    let retained = runner.get_ui_context();
    runner.set_ui_context(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let remove = runner.on_error(Rc::new(move |error| {
        output.borrow_mut().push(error.clone());
        Ok(())
    }));
    let report = loaded.api.ui_error_reporter(&retained).unwrap();
    drop(retained);
    report("custom.visibility", &"hidden failure".into()).unwrap();
    let expected = maestro_extensions::ExtensionError {
        extension_path: "/original-extension".into(),
        event: "custom.visibility".into(),
        error: "hidden failure".into(),
        stack: None,
    };
    assert_eq!(*seen.borrow(), std::slice::from_ref(&expected));
    runner.emit_error(&expected).unwrap();
    assert_eq!(*seen.borrow(), [expected.clone(), expected]);
    remove();
    report("custom.visibility", &"hidden failure".into()).unwrap();
    assert_eq!(seen.borrow().len(), 2);
    let _remove = runner.on_error(Rc::new(|_| Err("listener refused".into())));
    assert_eq!(
        report("custom.visibility", &"hidden failure".into())
            .unwrap_err()
            .to_string(),
        "listener refused"
    );
}
