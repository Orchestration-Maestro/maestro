//! Custom factories and their results run through the production artifact adapter.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionUIContext,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture, OverlayOptionsSource,
    },
    widgets::SizeValue,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A host that constructs custom components and closes them through their retained input.
struct Ui {
    /// The terminal lent to every custom factory.
    tui: maestro_tui::TUI,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = RefCell::new(Some(send));
        let closed = Cell::new(false);
        let close = Rc::new(move || {
            if closed.replace(true) {
                return Ok(false);
            }
            if let Some(send) = send.borrow_mut().take() {
                let _ = send.send(());
            }
            Ok(true)
        });
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let building = factory.call((self.tui.clone(), self.theme(), keys, close));
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            assert_eq!(built.width, Some(13.0));
            assert_eq!(built.component.try_render(20).unwrap(), ["custom 23"]);
            built
                .component
                .input_handler()
                .unwrap()
                .try_handle_input("commit")
                .map_err(|error| EventError::from(error.to_string()))?;
            receive
                .await
                .map_err(|error| EventError::from(error.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}

#[test]
fn canonical_custom_component_keeps_ready_and_async_factory_results() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _) = super::canonical_ui::screen();
        runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Ui { tui }))));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for (argument, answer) in [
            ("ui custom-ready", "custom ready done"),
            ("ui custom-async", "custom async done"),
        ] {
            assert_eq!(
                handler(argument.into(), runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                answer
            );
        }
        bus.clear();
    });
}

/// A host that shows custom overlays on the real frame writer in the application's order:
/// construction, placement, show, then the handle, and drives the shown overlay.
struct OverlayUi {
    /// The real frame writer.
    tui: maestro_tui::TUI,
}
impl ExtensionUIContextPort for OverlayUi {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = RefCell::new(Some(send));
        let close = Rc::new(move || {
            if let Some(send) = send.borrow_mut().take() {
                let _ = send.send(());
            }
            Ok(true)
        });
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let tui = self.tui.clone();
        let building = factory.call((tui.clone(), self.theme(), keys, close));
        Box::pin(async move {
            let CustomFactoryOutcome::Ready(built) = building? else {
                panic!("the overlay factory is ready");
            };
            let options = options.unwrap();
            assert_eq!(options.overlay, Some(true));
            let placement = match options.overlay_options.unwrap() {
                OverlayOptionsSource::Static(placement) => placement,
                OverlayOptionsSource::Resolver(resolve) => resolve.call(())?,
            };
            assert_eq!(placement.borrow().width, Some(SizeValue::Cells(30)));
            let shown = tui
                .show_overlay(built.component.clone(), Some(placement))
                .unwrap();
            let shown = Rc::new(RefCell::new(shown));
            options.on_handle.unwrap().call(Rc::clone(&shown))?;
            drive(&tui, &built.component);
            receive
                .await
                .map_err(|error| EventError::from(error.to_string()))?;
            shown.borrow_mut().hide().unwrap();
            Ok(())
        })
    }
}

/// Moves the shown overlay through its handle and its visibility check.
fn drive(tui: &maestro_tui::TUI, component: &maestro_extensions::ComponentHandle) {
    let input = |data: &str| {
        component
            .input_handler()
            .unwrap()
            .try_handle_input(data)
            .unwrap();
    };
    let rows = || component.try_render(40).unwrap();
    assert!(tui.has_overlay());
    assert_eq!(rows(), ["overlay focused=true hidden=false"]);
    input("hide");
    assert!(!tui.has_overlay());
    assert_eq!(rows(), ["overlay focused=false hidden=true"]);
    input("show");
    assert!(tui.has_overlay());
    input("unfocus");
    assert_eq!(rows(), ["overlay focused=false hidden=false"]);
    input("focus");
    assert_eq!(rows(), ["overlay focused=true hidden=false"]);
    input("fail");
    assert!(!tui.has_overlay());
    assert_eq!(rows(), ["overlay focused=true hidden=false"]);
    input("commit");
}

#[test]
fn canonical_custom_overlay_hands_over_its_handle_and_reports_visibility_failures() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _) = super::canonical_ui::screen();
        runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(OverlayUi { tui }))));
        let errors = Rc::new(RefCell::new(Vec::new()));
        let received = errors.clone();
        let _remove = runner.on_error(Rc::new(move |error| {
            received.borrow_mut().push(error.clone());
            Ok(())
        }));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for placement in ["static", "resolved"] {
            assert_eq!(
                handler(
                    format!("ui custom-overlay {placement}"),
                    runner.create_command_context()
                )
                .await
                .unwrap_err()
                .to_string(),
                "overlay done"
            );
        }
        let expected = maestro_extensions::ExtensionError {
            extension_path: std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
            event: "overlay.visible".into(),
            error: "visibility failed at 40x12".into(),
            stack: None,
        };
        assert_eq!(*errors.borrow(), [expected.clone(), expected]);
        bus.clear();
    });
}
