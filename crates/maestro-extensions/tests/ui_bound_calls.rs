//! Bound UI callbacks: both execution families, origin, identity and release.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::error_support::Marker;
use crate::local_support::{Loaded, Released, Trace, loaded_for, mark};
use crate::ui_toolkit::{Keyboard, Metadata, Words, screen};
use maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider;
use maestro_extensions::types::{
    AutocompleteProviderFactory, ComponentDisposer, ComponentFactory, CustomClose,
    CustomComponentFactory, CustomDone, CustomFactory, CustomFactoryOutcome, EditorFactoryOutcome,
    ExtensionComponent, ExtensionKeybindings, FooterFactory, OverlayHandleCallback,
    OverlayOptionsResolver,
};
use maestro_extensions::{
    CallbackEmitter, EventBus, EventListener, ExtensionAPI, ExtensionExecution, ExtensionUIContext,
    TerminalInputHandler, create_event_bus, create_event_bus_for, types::UIHostCall,
};
use maestro_models::{Cancellation, Local, Threaded};
use maestro_theme::{LiveTheme, get_editor_theme};
use maestro_tui::tui::{InputListener, InputListenerResult};
use maestro_tui::{
    AutocompleteProvider, Editor, EditorComponent, EditorOptions, EditorTheme, Input,
    KeybindingsManager, OverlayHandle, OverlayOptions, TUI, TUI_KEYBINDINGS,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

/// A shown overlay's handle, shared by host and extension.
type SharedOverlay = Rc<RefCell<Box<dyn OverlayHandle>>>;

/// The toolkit and theme operands every UI callback receives.
struct Operands {
    /// The real terminal this fixture writes to.
    tui: TUI,
    /// The metadata a footer factory reads.
    data: Rc<dyn ReadonlyFooterDataProvider>,
    /// The provider an autocomplete factory wraps.
    provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    /// What the fixture callbacks consumed.
    seen: Trace,
}

impl Operands {
    /// Real operands over a fresh recording terminal and live metadata.
    fn new() -> Self {
        let watch = Rc::new(Released(Rc::new(Cell::new(false))));
        Self {
            tui: screen().0,
            data: Rc::new(Metadata::new(watch)),
            provider: Rc::new(Words),
            seen: Trace::default(),
        }
    }
}

/// Every UI callback alias the extension API offers, built through one facade.
struct Families {
    /// Raw terminal input.
    input: TerminalInputHandler,
    /// Explicit component disposal.
    dispose: ComponentDisposer,
    /// A widget or header component.
    component: ComponentFactory,
    /// A custom footer.
    footer: FooterFactory,
    /// An autocomplete wrapper.
    autocomplete: AutocompleteProviderFactory,
    /// A replacement editor.
    editor: maestro_extensions::EditorFactory,
    /// Overlay options resolved by the host.
    resolver: OverlayOptionsResolver,
    /// The shown overlay's handle.
    handle: OverlayHandleCallback,
    /// A typed custom component.
    custom: CustomFactory<String>,
    /// A custom component the host closes.
    custom_component: CustomComponentFactory,
}

/// Builds the widget, footer and editor aliases through `api`.
fn widget_families<E: ExtensionExecution>(
    api: &ExtensionAPI<E>,
    seen: &Trace,
) -> (
    TerminalInputHandler,
    ComponentDisposer,
    ComponentFactory,
    FooterFactory,
    AutocompleteProviderFactory,
) {
    let seen = seen.clone();
    (
        api.ui_callback(|data: String, _| Ok(InputListenerResult::Replace(data))),
        {
            let seen = seen.clone();
            api.ui_callback(move |(), _| {
                mark(&seen, "disposed");
                Ok(())
            })
        },
        {
            let seen = seen.clone();
            api.ui_callback(move |(tui, _): (TUI, LiveTheme), _| {
                let input = Rc::new(Input::new());
                input.set_value("widget".into());
                tui.add_child(input.clone());
                mark(
                    &seen,
                    format!("component {}", tui.children().borrow().len()),
                );
                Ok(ExtensionComponent {
                    component: input,
                    dispose: None,
                    set_expanded: None,
                    width: None,
                })
            })
        },
        {
            let seen = seen.clone();
            api.ui_callback(
                move |(_, _, data): (TUI, LiveTheme, Rc<dyn ReadonlyFooterDataProvider>), _| {
                    let branch = data.get_git_branch().unwrap_or_default();
                    mark(&seen, format!("footer {branch}"));
                    Ok(ExtensionComponent {
                        component: Rc::new(Input::new()),
                        dispose: None,
                        set_expanded: None,
                        width: None,
                    })
                },
            )
        },
        {
            api.ui_callback(
                move |current: Rc<dyn AutocompleteProvider<Signal = Cancellation>>, _| {
                    mark(&seen, "autocomplete");
                    Ok(current)
                },
            )
        },
    )
}

/// Builds every alias through `api`, recording what each callback consumed.
fn families<E: ExtensionExecution>(api: &ExtensionAPI<E>, operands: &Operands) -> Families {
    let seen = operands.seen.clone();
    let (input, dispose, component, footer, autocomplete) = widget_families(api, &seen);
    let (custom, custom_component) = custom_families(api, &seen);
    Families {
        input,
        dispose,
        component,
        footer,
        autocomplete,
        editor: {
            let seen = seen.clone();
            api.ui_callback(
                move |(tui, theme, _): (TUI, EditorTheme, ExtensionKeybindings), _| {
                    let editor: Rc<dyn EditorComponent<Signal = Cancellation>> =
                        Rc::new(Editor::new(&tui, theme, EditorOptions::default()));
                    mark(&seen, "editor");
                    Ok(EditorFactoryOutcome::Editor(editor))
                },
            )
        },
        resolver: {
            let seen = seen.clone();
            api.ui_callback(move |(), _| {
                mark(&seen, "resolver");
                Ok(Rc::new(RefCell::new(OverlayOptions {
                    min_width: Some(7),
                    ..OverlayOptions::default()
                })))
            })
        },
        handle: {
            let seen = seen.clone();
            api.ui_callback(move |handle: SharedOverlay, _| {
                handle.borrow_mut().set_hidden(true);
                mark(&seen, "handle");
                Ok(())
            })
        },
        custom,
        custom_component,
    }
}

/// Builds the typed and host-closed custom component aliases through `api`.
fn custom_families<E: ExtensionExecution>(
    api: &ExtensionAPI<E>,
    seen: &Trace,
) -> (CustomFactory<String>, CustomComponentFactory) {
    let seen = seen.clone();
    (
        {
            let seen = seen.clone();
            api.ui_callback(
                move |(_, _, _, done): (
                    TUI,
                    LiveTheme,
                    ExtensionKeybindings,
                    CustomDone<String>,
                ),
                      _| {
                    done("committed".into())?;
                    mark(&seen, "custom");
                    Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                        component: Rc::new(Input::new()),
                        dispose: None,
                        set_expanded: None,
                        width: None,
                    }))
                },
            )
        },
        {
            api.ui_callback(
                move |(_, _, _, close): (TUI, LiveTheme, ExtensionKeybindings, CustomClose), _| {
                    let accepted = close()?;
                    mark(&seen, format!("custom_component {accepted}"));
                    Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                        component: Rc::new(Input::new()),
                        dispose: None,
                        set_expanded: None,
                        width: None,
                    }))
                },
            )
        },
    )
}

/// Toolkit defaults supplied without an application reload.
fn toolkit_keybindings() -> ExtensionKeybindings {
    ExtensionKeybindings {
        manager: KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new()),
        reload: None,
    }
}

/// Invokes every constructed alias with the real operands.
fn invoke_all(
    built: &Families,
    operands: &Operands,
    overlay: &SharedOverlay,
    done: CustomDone<String>,
    close: CustomClose,
) {
    let theme = LiveTheme::default();
    let editor_theme = get_editor_theme(theme.clone());
    let keys = toolkit_keybindings();
    let component = built
        .component
        .call((operands.tui.clone(), theme.clone()))
        .unwrap();
    assert!(
        operands
            .tui
            .children()
            .borrow()
            .iter()
            .any(|child| Rc::ptr_eq(child, &component.component))
    );
    component
        .dispose
        .map(|dispose| dispose.call(()))
        .transpose()
        .unwrap();
    let footer = built
        .footer
        .call((
            operands.tui.clone(),
            theme.clone(),
            Rc::clone(&operands.data),
        ))
        .unwrap();
    footer.component.input_handler().unwrap().handle_input("!");
    let wrapped = built
        .autocomplete
        .call(Rc::clone(&operands.provider))
        .unwrap();
    assert!(Rc::ptr_eq(&wrapped, &operands.provider));
    let editor = built
        .editor
        .call((operands.tui.clone(), editor_theme, keys.clone()))
        .unwrap();
    editor.set_text("typed");
    assert_eq!(editor.get_text(), "typed");
    let resolved = built.resolver.call(()).unwrap();
    assert_eq!(resolved.borrow().min_width, Some(7));
    built.handle.call(Rc::clone(overlay)).unwrap();
    assert!(overlay.borrow().is_hidden());
    let ready = built
        .custom
        .call((operands.tui.clone(), theme.clone(), keys.clone(), done))
        .unwrap();
    assert!(matches!(ready, CustomFactoryOutcome::Ready(_)));
    let closing = built
        .custom_component
        .call((operands.tui.clone(), theme, keys, close))
        .unwrap();
    assert!(matches!(closing, CustomFactoryOutcome::Ready(_)));
    assert!(matches!(
        built.input.call("x".into()),
        Ok(InputListenerResult::Replace(data)) if data == "x"
    ));
    built.dispose.call(()).unwrap();
}

/// Builds and invokes every alias for one execution family.
fn family_case<E: ExtensionExecution>(loaded: &Loaded<E>) {
    let operands = Operands::new();
    let built = families(&loaded.api, &operands);
    let committed = Rc::new(RefCell::new(Vec::new()));
    let slot = committed.clone();
    let done: CustomDone<String> = Rc::new(move |value| {
        slot.borrow_mut().push(value);
        Ok(())
    });
    let closed = Rc::new(Cell::new(false));
    let gate = closed.clone();
    let close: CustomClose = Rc::new(move || Ok(!gate.replace(true)));
    let overlay = Rc::new(RefCell::new(
        operands
            .tui
            .show_overlay(Rc::new(Input::new()), None)
            .unwrap(),
    ));
    invoke_all(&built, &operands, &overlay, done, close);
    assert_eq!(*committed.borrow(), ["committed"]);
    assert!(closed.get());
    assert_eq!(
        *operands.seen.borrow(),
        [
            "component 1",
            "footer main",
            "autocomplete",
            "editor",
            "resolver",
            "handle",
            "custom",
            "custom_component true",
            "disposed",
        ]
    );
}

#[test]
fn ui_callback_families_bind_both_execution_families() {
    let controller = create_event_bus(std::sync::Arc::new(|_| {}), std::sync::Arc::new(|_| {}));
    family_case(&loaded_for::<Threaded>(
        "/threaded",
        controller.for_extension(),
    ));
    let local = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    family_case(&loaded_for::<Local>("/local", local.for_extension()));
}

/// A trace both execution families can record into.
trait Recorder: Clone + 'static {
    /// Records one label.
    fn record(&self, label: String);
    /// The recorded labels.
    fn lines(&self) -> Vec<String>;
}

impl Recorder for Trace {
    fn record(&self, label: String) {
        mark(self, label);
    }
    fn lines(&self) -> Vec<String> {
        self.borrow().clone()
    }
}

/// A trace the default thread-safe family can record into.
#[derive(Clone, Default)]
struct SharedTrace(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

/// A recorder the default thread-safe family can retain.
trait SharedRecorder: Recorder + Send + Sync {}

impl SharedRecorder for SharedTrace {}

impl Recorder for SharedTrace {
    fn record(&self, label: String) {
        self.0.lock().unwrap().push(label);
    }
    fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// A listener recording one label in the default family.
fn threaded_observer<R: SharedRecorder>(trace: &R, label: &'static str) -> EventListener {
    let trace = trace.clone();
    std::sync::Arc::new(move |_: std::sync::Arc<Value>, _: CallbackEmitter<'_>| {
        trace.record(label.to_owned());
        Ok(None)
    })
}

/// A listener recording one label in the local family.
fn local_observer<R: Recorder>(trace: &R, label: &'static str) -> EventListener<Local> {
    let trace = trace.clone();
    Rc::new(
        move |_: std::sync::Arc<Value>, _: CallbackEmitter<'_, Local>| {
            trace.record(label.to_owned());
            Ok(None)
        },
    )
}

/// Binds an input callback, releases its facade, then drives the host.
fn origin_case<E: ExtensionExecution, R: Recorder>(
    loaded: Loaded<E>,
    foreign: &EventBus<E>,
    observers: (EventListener<E>, EventListener<E>),
    trace: &R,
) {
    let (foreign_listener, same_listener) = observers;
    foreign.on("ui", foreign_listener);
    loaded.api.events.on("ui", same_listener);
    let inner = trace.clone();
    let failure = std::sync::Arc::new(());
    let returned = std::sync::Arc::clone(&failure);
    let handler: TerminalInputHandler = loaded.api.ui_callback(move |data: String, emitter| {
        inner.record("before".to_owned());
        emitter.emit("ui", json!(null));
        assert_eq!(inner.lines(), ["before", "same"]);
        inner.record(format!("after {data}"));
        Err(Box::new(Marker(std::sync::Arc::as_ptr(&returned).addr())))
    });
    let (tui, terminal) = screen();
    tui.start().unwrap();
    let host = Rc::new(Keyboard::new(tui));
    let ui = ExtensionUIContext::new(host.clone());
    ui.on_terminal_input(handler).unwrap();
    drop(loaded);
    terminal.send_input("data");
    assert_eq!(trace.lines(), ["before", "same", "after data", "foreign"]);
    let errors = host.errors.borrow();
    assert_eq!(errors.len(), 1);
    let reported = errors[0]
        .downcast_ref::<Marker<usize>>()
        .expect("the host reported a rewritten error");
    assert_eq!(reported.0, std::sync::Arc::as_ptr(&failure).addr());
}

#[test]
fn ui_callback_origin_survives_host_transfer_and_callback_error() {
    let threaded = create_event_bus(std::sync::Arc::new(|_| {}), std::sync::Arc::new(|_| {}));
    let threaded_trace = SharedTrace::default();
    let threaded_loaded = loaded_for::<Threaded>("/threaded", threaded.for_extension());
    origin_case(
        threaded_loaded,
        &threaded.for_extension(),
        (
            threaded_observer(&threaded_trace, "foreign"),
            threaded_observer(&threaded_trace, "same"),
        ),
        &threaded_trace,
    );
    let local = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let local_trace = Trace::default();
    let local_loaded = loaded_for::<Local>("/local", local.for_extension());
    origin_case(
        local_loaded,
        &local.for_extension(),
        (
            local_observer(&local_trace, "foreign"),
            local_observer(&local_trace, "same"),
        ),
        &local_trace,
    );
}

/// An operand and result that are neither `Clone` nor `Copy`.
struct Opaque(u8);

fn clone_free_case<E: ExtensionExecution>(api: &ExtensionAPI<E>) {
    let call: UIHostCall<Opaque, Opaque> =
        api.ui_callback(|args: Opaque, _| Ok(Opaque(args.0 + 1)));
    let shared = call.clone();
    drop(call);
    assert_eq!(shared.call(Opaque(4)).unwrap().0, 5);
}

#[test]
fn bound_ui_calls_do_not_require_cloneable_operands_or_results() {
    let threaded = create_event_bus(std::sync::Arc::new(|_| {}), std::sync::Arc::new(|_| {}));
    let threaded_loaded = loaded_for::<Threaded>("/threaded", threaded.for_extension());
    clone_free_case(&threaded_loaded.api);
    let local = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let local_loaded = loaded_for::<Local>("/local", local.for_extension());
    clone_free_case(&local_loaded.api);
}

#[test]
fn bound_ui_call_identity_and_hash_follow_the_callback_allocation() {
    let local = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let loaded = loaded_for::<Local>("/identity", local.for_extension());
    let input: TerminalInputHandler = loaded
        .api
        .ui_callback(|_: String, _| Ok(InputListenerResult::Pass));
    let other: TerminalInputHandler = loaded
        .api
        .ui_callback(|_: String, _| Ok(InputListenerResult::Pass));
    let shared: TerminalInputHandler = loaded
        .api
        .ui_callback(|data: String, _| Ok(InputListenerResult::Replace(data)));
    assert!(input.ptr_eq(&input.clone()));
    assert!(!input.ptr_eq(&other));
    assert!(!shared.ptr_eq(&input));
    let mut counts: HashMap<TerminalInputHandler, usize> = HashMap::new();
    for _ in 0..2 {
        *counts.entry(input.clone()).or_insert(0) += 1;
    }
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[&input], 2);
    let mut listeners: HashMap<TerminalInputHandler, InputListener> = HashMap::new();
    let mut created = 0;
    for _ in 0..2 {
        if !listeners.contains_key(&input) {
            let handler = input.clone();
            listeners.insert(
                input.clone(),
                Rc::new(move |data: &str| {
                    handler
                        .call(data.to_owned())
                        .unwrap_or(InputListenerResult::Pass)
                }),
            );
            created += 1;
        }
    }
    assert_eq!((listeners.len(), created), (1, 1));
    let listener = listeners.remove(&input).unwrap();
    assert!(matches!(listener("data"), InputListenerResult::Pass));
    assert!(listeners.is_empty());
}

#[test]
fn bound_ui_calls_release_the_origin_after_the_last_handle() {
    let local = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let loaded = loaded_for::<Local>("/release", local.for_extension());
    let weak = {
        let payload = Rc::new(7_u8);
        let weak = Rc::downgrade(&payload);
        let call: UIHostCall<(), u8> = loaded.api.ui_callback(move |(), _| Ok(*payload));
        let shared = call.clone();
        drop(call);
        assert!(weak.upgrade().is_some());
        assert_eq!(shared.call(()).unwrap(), 7);
        drop(shared);
        weak
    };
    assert!(weak.upgrade().is_none());
    let released = Rc::new(Cell::new(false));
    let sentinel = Released(Rc::clone(&released));
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| {}),
        Rc::new(move |_: &str| {
            let _ = &sentinel;
        }),
    );
    let origin = loaded_for::<Local>("/origin", bus.for_extension());
    let handle: UIHostCall<(), ()> = origin.api.ui_callback(|(), _| Ok(()));
    drop(origin);
    drop(bus);
    assert!(
        !released.get(),
        "a handle released its origin while it could still be called"
    );
    handle.call(()).unwrap();
    drop(handle);
    assert!(released.get(), "the origin outlived its last handle");
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let failing: UIHostCall<(), u8> = loaded.api.ui_callback(move |(), _| {
        counted.set(counted.get() + 1);
        Err("failed".into())
    });
    for _ in 0..2 {
        assert_eq!(failing.call(()).unwrap_err().to_string(), "failed");
    }
    assert_eq!(calls.get(), 2);
    drop(loaded);
    assert_eq!(failing.call(()).unwrap_err().to_string(), "failed");
}
