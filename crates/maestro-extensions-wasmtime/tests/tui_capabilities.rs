//! Retained terminal capabilities reach the real frame writer.
#![cfg(test)]

use maestro_extensions::{
    EventError, ExtensionUIContext, ExtensionWidgetOptions,
    types::{ExtensionComponent, ExtensionUIContextPort, WidgetContent},
};
use std::{cell::RefCell, rc::Rc};

/// The application's original terminal and current widget.
struct Ui {
    /// Frame writer lent to the author.
    tui: maestro_tui::TUI,
    /// Shown component.
    shown: RefCell<Option<ExtensionComponent>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn set_widget(
        &self,
        _: String,
        content: Option<WidgetContent>,
        _: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        if let Some(WidgetContent::Factory(factory)) = content {
            self.shown
                .replace(Some(factory.call((self.tui.clone(), self.theme()))?));
        } else {
            self.shown.take();
        }
        Ok(())
    }
}

#[test]
fn canonical_tui_reads_the_original_frame_writer_redraw_count() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = super::canonical_ui::screen();
        let ui = Rc::new(Ui {
            tui,
            shown: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        assert_eq!(
            handler("ui tui".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "terminal shown"
        );
        let component = ui.shown.borrow().as_ref().unwrap().component.clone();
        assert_eq!(component.try_render(7).unwrap(), ["redraws=0"]);
        ui.tui.request_render(true);
        ui.tui.render_pending_frame().unwrap();
        assert_eq!(ui.tui.full_redraws(), 1);
        assert_eq!(component.try_render(11).unwrap(), ["redraws=1"]);
        bus.clear();
    });
}
