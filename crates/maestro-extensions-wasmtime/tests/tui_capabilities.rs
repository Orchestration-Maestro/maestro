//! Retained terminal capabilities reach the real frame writer.
#![cfg(test)]

use maestro_extensions::{
    EventError, ExtensionUIContext, ExtensionWidgetOptions,
    types::{ExtensionComponent, ExtensionUIContextPort, WidgetContent},
};
use serde_json::json;
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

#[test]
fn canonical_tui_delivers_focus_and_blur_to_the_authored_component_in_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let seen = Rc::new(RefCell::new(Vec::new()));
        let changes = seen.clone();
        bus.on(
            "focus-changed",
            Rc::new(move |value, _| {
                changes.borrow_mut().push((*value).clone());
                Ok(None)
            }),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, terminal) = super::canonical_ui::screen();
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
            handler("ui focus".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "focus shown"
        );
        let field = ui.shown.borrow().as_ref().unwrap().component.clone();
        assert!(field.focusable().is_some());
        ui.tui.set_focus(Some(field.clone()));
        assert_eq!(*seen.borrow(), [json!(true)]);
        assert_eq!(
            field.try_render(10).unwrap(),
            ["field\x1b_maestro:c\x07 [true]"]
        );
        cursor(&ui.tui, &terminal, field.clone());
        ui.tui.set_focus(None);
        ui.tui.set_focus(Some(field.clone()));
        assert_eq!(*seen.borrow(), [json!(true), json!(false), json!(true)]);
        assert_eq!(
            field.try_render(10).unwrap(),
            ["field\x1b_maestro:c\x07 [true, false, true]"]
        );
        bus.clear();
    });
}

/// The real frame writer consumes the guest's marker and shows the hardware cursor.
fn cursor(
    tui: &maestro_tui::TUI,
    terminal: &super::recording_terminal::RecordingTerminal,
    field: maestro_extensions::ComponentHandle,
) {
    tui.add_child(field);
    tui.set_show_hardware_cursor(true).unwrap();
    tui.request_render(true);
    tui.render_pending_frame().unwrap();
    let output = terminal.writes().concat();
    assert!(!output.contains("maestro:c"));
    assert!(output.contains("\x1b[?25h"));
}

#[test]
fn canonical_host_editors_keep_their_component_focus_capability() {
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
            handler("ui editor-focus".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "editor focus shown"
        );
        let field = ui.shown.borrow().as_ref().unwrap().component.clone();
        assert!(!field.focusable().unwrap().focus_flag().get());
        ui.tui.set_focus(Some(field.clone()));
        assert!(field.focusable().unwrap().focus_flag().get());
        ui.tui.set_focus(None);
        assert!(!field.focusable().unwrap().focus_flag().get());
        bus.clear();
    });
}

#[test]
fn canonical_overlay_options_retain_the_native_placement_owner() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        assert_eq!(
            handler("ui overlay-options".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "overlay options shared"
        );
        bus.clear();
    });
}

/// Delivers `command` to the driver widget and returns the row it answers.
fn run(driver: &maestro_extensions::ComponentHandle, command: &str) -> String {
    driver
        .input_handler()
        .unwrap()
        .try_handle_input(command)
        .unwrap();
    last(driver)
}

/// The driver widget's last row.
fn last(driver: &maestro_extensions::ComponentHandle) -> String {
    driver.try_render(40).unwrap().last().cloned().unwrap()
}

/// Frame writer components, flags, overlays and focus reach the original frame writer.
fn frame(ui: &Ui, driver: &maestro_extensions::ComponentHandle) {
    assert_eq!(run(driver, "children"), r#"rows ["child"]"#);
    assert_eq!(
        maestro_extensions::Component::try_render(&ui.tui, 20).unwrap(),
        ["child"]
    );
    assert_eq!(run(driver, "clear"), "rows []");
    assert_eq!(run(driver, "settings"), "cursor true shrink true");
    assert!(ui.tui.get_show_hardware_cursor() && ui.tui.get_clear_on_shrink());
    assert_eq!(run(driver, "overlay"), "overlay true focused true");
    assert!(ui.tui.has_overlay());
    assert_eq!(run(driver, "focus"), "focus false true");
    assert_eq!(run(driver, "hide"), "overlay false");
    assert!(!ui.tui.has_overlay());
}

/// Terminal requests write their exact bytes and read the live size.
fn terminal(
    driver: &maestro_extensions::ComponentHandle,
    terminal: &super::recording_terminal::RecordingTerminal,
) {
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let tapped = bytes.clone();
    terminal.set_tap(move |data| tapped.borrow_mut().push(data.to_owned()));
    assert_eq!(run(driver, "terminal"), "terminal 40x12 kitty true");
    assert_eq!(
        *bytes.borrow(),
        [
            "literal",
            "\x1b[2A",
            "\x1b[?25l",
            "\x1b[?25h",
            "\x1b[K",
            "\x1b[J",
            "\x1b[2J\x1b[H",
            "\x1b]0;title\x07",
            "\x1b]9;4;3\x07",
        ]
    );
    terminal.resize(50, 9);
    assert_eq!(run(driver, "terminal"), "terminal 50x9 kitty true");
}

/// Input listeners and the debug key see terminal input while the frame writer runs.
fn input(
    ui: &Ui,
    driver: &maestro_extensions::ComponentHandle,
    terminal: &super::recording_terminal::RecordingTerminal,
) {
    assert_eq!(run(driver, "start"), "started");
    assert_eq!(run(driver, "listen"), "listening");
    terminal.send_input("x");
    assert_eq!(last(driver), "listener x");
    assert_eq!(run(driver, "debug"), "debug set");
    terminal.send_input("\x1b[100;6u");
    let rows = driver.try_render(40).unwrap();
    assert_eq!(
        rows[rows.len() - 3..],
        ["debug set", "listener \x1b[100;6u", "debug key"]
    );
    assert_eq!(run(driver, "unlisten"), "not listening");
    terminal.send_input("x");
    assert_eq!(last(driver), "not listening");
    let redraws = ui.tui.full_redraws();
    assert_eq!(run(driver, "render"), "render requested");
    ui.tui.render_pending_frame().unwrap();
    assert!(ui.tui.full_redraws() > redraws);
    assert_eq!(run(driver, "stop"), "stopped");
    assert_eq!(terminal.events(), ["start", "stop"]);
}

/// The answer of one bridge command.
type Answer = std::pin::Pin<Box<dyn std::future::Future<Output = String>>>;

/// Shows the driver widget on a real frame writer and runs `body` with it, the bridge's
/// commands and the runner errors reported meanwhile.
fn with_writer(
    body: impl AsyncFnOnce(
        &Ui,
        &maestro_extensions::ComponentHandle,
        &super::recording_terminal::RecordingTerminal,
        &dyn Fn(&str) -> Answer,
    ) -> Vec<maestro_extensions::ExtensionError>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, recording) = super::canonical_ui::screen();
        let ui = Rc::new(Ui {
            tui,
            shown: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
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
        let command = |command: &str| -> Answer {
            let answer = handler(command.into(), runner.create_command_context());
            Box::pin(async move { answer.await.unwrap_err().to_string() })
        };
        assert_eq!(command("ui writer").await, "writer shown");
        assert_eq!(command("ui writer-drain").await, "drained");
        let driver = ui.shown.borrow().as_ref().unwrap().component.clone();
        let command: &dyn Fn(&str) -> Answer = &command;
        let expected = body(ui.as_ref(), &driver, &recording, command).await;
        assert_eq!(*errors.borrow(), expected);
        bus.clear();
    });
}

#[test]
fn canonical_tui_writer_forwards_frame_and_terminal_requests() {
    with_writer(async |ui, driver, recording, _| {
        frame(ui, driver);
        terminal(driver, recording);
        input(ui, driver, recording);
        Vec::new()
    });
}

#[test]
fn canonical_tui_writer_input_subscription_releases_its_listener() {
    with_writer(async |_, driver, terminal, _| {
        assert_eq!(run(driver, "start"), "started");
        assert_eq!(run(driver, "resubscribe"), "resubscribed released true");
        terminal.send_input("x");
        assert_eq!(last(driver), "listener x");
        assert_eq!(run(driver, "unlisten"), "not listening");
        terminal.send_input("x");
        assert_eq!(last(driver), "not listening");
        assert_eq!(run(driver, "stop"), "stopped");
        Vec::new()
    });
}

#[test]
fn canonical_tui_writer_releases_retired_focus_and_overlay_targets() {
    with_writer(async |_, driver, _, command| {
        assert_eq!(run(driver, "retire"), "retired");
        // The host releases retired identities through the guest's queue; a later command
        // runs after them.
        assert_eq!(command("ui writer-drain").await, "drained");
        assert_eq!(run(driver, "released"), "released true");
        Vec::new()
    });
}

#[test]
fn canonical_tui_writer_reports_direct_overlay_visibility_failures() {
    with_writer(async |_, driver, _, _| {
        assert_eq!(run(driver, "hidden"), "hidden overlay false");
        let failed = maestro_extensions::ExtensionError {
            extension_path: std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
            event: "overlay.visible".into(),
            error: "direct overlay failed at 40x12".into(),
            stack: None,
        };
        // Showing the overlay and asking for one each check its visibility once.
        vec![failed.clone(), failed]
    });
}
