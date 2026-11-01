//! Extension UI and renderer records through the application root.
use maestro_app::presentation_data::footer_data_provider::{
    ExtensionStatusIter, ExtensionStatuses,
};
use maestro_app::{
    AutocompleteProviderFactory, Cancellation, ExtensionUIContext, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer, ReadonlyFooterDataProvider,
    SharedHandle, WidgetPlacement, WorkingIndicatorOptions,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::{
    Extension, ExtensionAPI, Local, SourceInfo, SourceOrigin, SourceScope, TerminalInputHandler,
    Threaded, UIHostCall,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_tui::tui::InputListenerResult;
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, RwLock};

/// Returns its operand as the extension owner's declaration.
fn native_ui(ui: ExtensionUIContext) -> maestro_extensions::ExtensionUIContext {
    ui
}

/// Returns the footer view the application provider implements.
fn extension_footer(
    provider: &dyn ReadonlyFooterDataProvider,
) -> &dyn maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider {
    provider
}

/// Returns the input alias as the extension owner's declaration.
fn native_input(
    handler: maestro_app::TerminalInputHandler,
) -> maestro_extensions::TerminalInputHandler {
    handler
}

/// Returns the autocomplete alias as the extension owner's declaration.
fn native_wrapper(
    wrap: AutocompleteProviderFactory,
) -> maestro_extensions::AutocompleteProviderFactory {
    wrap
}

/// Provenance for a facade the application constructs itself.
#[cfg(not(target_arch = "wasm32"))]
fn source() -> Arc<RwLock<SourceInfo>> {
    Arc::new(RwLock::new(SourceInfo {
        path: "app".into(),
        source: "app".into(),
        scope: SourceScope::Temporary,
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    }))
}

#[test]
fn native_ui_exports_share_the_existing_records() {
    let remove: maestro_app::UIUnsubscribe = Rc::new(|| Ok(()));
    let owner: maestro_extensions::types::UIUnsubscribe = remove;
    let mut listeners = maestro_app::Listeners::default();
    listeners.add(owner);
    listeners.snapshot()[0]().unwrap();
    let options = MessageRenderOptions { expanded: true };
    let session: maestro_session::messages::MessageRenderOptions = options;
    let _: maestro_extensions::MessageRenderOptions = session;
    let renderer: MessageRenderer = SharedHandle::new(|_, _, _, _| Ok(None));
    let _: maestro_extensions::MessageRenderer = renderer;
    let signal = Cancellation::new();
    let dialog = ExtensionUIDialogOptions {
        signal: Some(signal.clone()),
        timeout: Some(-0.5),
    };
    signal.abort();
    assert!(dialog.signal.as_ref().is_some_and(Cancellation::is_aborted));
    let widget = ExtensionWidgetOptions {
        placement: Some(WidgetPlacement::BelowEditor),
    };
    let _: maestro_extensions::ExtensionWidgetOptions = widget;
    let indicator = WorkingIndicatorOptions {
        frames: Some(Vec::new()),
        interval_ms: None,
    };
    let _: maestro_extensions::WorkingIndicatorOptions = indicator;
    let _ = (native_ui, extension_footer, native_input, native_wrapper);
    let statuses: maestro_extensions::footer_data_provider::ExtensionStatuses =
        ExtensionStatuses::new();
    statuses.insert("build".into(), "passing".into());
    let walk: ExtensionStatusIter = statuses.iter();
    assert_eq!(
        walk.collect::<Vec<_>>(),
        [("build".to_owned(), "passing".to_owned())]
    );
}

// The browser build has no source scopes, and its thread-safe family shares through `Rc`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bound_ui_calls_cross_application_exports_in_both_families() {
    let threaded_events =
        maestro_app::create_event_bus(Arc::new(|_| {}), Arc::new(|_| {})).for_extension();
    let threaded_owner = Arc::new(RwLock::new(Extension::<Threaded>::new(
        "app".into(),
        "app".into(),
        source(),
    )));
    let threaded = ExtensionAPI::new(
        &threaded_owner,
        maestro_app::create_extension_runtime::<Threaded>(),
        threaded_events,
        ".".into(),
    );
    let local_events = maestro_app::create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}))
        .for_extension();
    let local_owner = Rc::new(RwLock::new(Extension::<Local>::new(
        "app".into(),
        "app".into(),
        source(),
    )));
    let local = ExtensionAPI::new(
        &local_owner,
        maestro_app::create_extension_runtime::<Local>(),
        local_events,
        ".".into(),
    );
    let input: TerminalInputHandler =
        threaded.ui_callback(|data: String, _| Ok(InputListenerResult::Replace(data)));
    let shared: maestro_extensions::TerminalInputHandler = input.clone();
    assert!(shared.ptr_eq(&input));
    assert!(matches!(
        input.call("x".into()),
        Ok(InputListenerResult::Replace(data)) if data == "x"
    ));
    let consumed: TerminalInputHandler =
        local.ui_callback(|_: String, _| Ok(InputListenerResult::Consume));
    let _: UIHostCall<String, InputListenerResult> = consumed;
    assert!(matches!(
        consumed.call("y".into()),
        Ok(InputListenerResult::Consume)
    ));
    let wrap: AutocompleteProviderFactory = local.ui_callback(|current, _| Ok(current));
    let _: maestro_extensions::AutocompleteProviderFactory = wrap;
}
