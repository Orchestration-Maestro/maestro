//! Extension UI and renderer records through the application root.
use maestro_app::presentation_data::footer_data_provider::{
    ExtensionStatusIter, ExtensionStatuses,
};
use maestro_app::{
    AutocompleteProviderFactory, Cancellation, ExtensionUIContext, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer, ReadonlyFooterDataProvider,
    SharedHandle, TerminalInputHandler, WidgetPlacement, WorkingIndicatorOptions,
};
use maestro_tui::tui::InputListenerResult;
use std::rc::Rc;

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

#[test]
fn native_ui_exports_share_the_existing_records() {
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
    let input: TerminalInputHandler = Rc::new(|data, _| Ok(InputListenerResult::Replace(data)));
    let _: maestro_extensions::TerminalInputHandler = input;
    let wrap: AutocompleteProviderFactory = Rc::new(|current, _| Ok(current));
    let _: maestro_extensions::AutocompleteProviderFactory = wrap;
    let _ = (native_ui, extension_footer);
    let statuses: maestro_extensions::footer_data_provider::ExtensionStatuses =
        ExtensionStatuses::new();
    statuses.insert("build".into(), "passing".into());
    let walk: ExtensionStatusIter = statuses.iter();
    assert_eq!(
        walk.collect::<Vec<_>>(),
        [("build".to_owned(), "passing".to_owned())]
    );
}
