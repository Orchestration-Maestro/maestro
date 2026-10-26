//! Custom factories and their results run through the production artifact adapter.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionUIContext,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture,
    },
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
        let keys = maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new());
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
