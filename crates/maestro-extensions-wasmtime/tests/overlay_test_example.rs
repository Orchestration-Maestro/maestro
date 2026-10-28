//! The shipped overlay demo shown over a conversation on the real frame writer and driven
//! by terminal input.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture, NotificationType,
    },
    widgets::ExtensionKeybindings,
};
use maestro_tui::{OverlayOptions, SizeValue, TUI, TerminalImage, extract_ansi_code};
use std::{cell::RefCell, path::Path, rc::Rc, sync::RwLock};
use tokio::sync::oneshot;

use super::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal};

/// Terminal columns: the 70-cell overlay leaves five on each side.
const COLUMNS: usize = 80;
/// Terminal rows, all filled by the conversation.
const ROWS: usize = 24;
/// Arrow keys as the terminal sends them.
const UP: &str = "\x1b[A";
/// The down arrow.
const DOWN: &str = "\x1b[B";
/// The right arrow.
const RIGHT: &str = "\x1b[C";
/// The left arrow.
const LEFT: &str = "\x1b[D";

/// Conversation text under the overlay, a dot in every cell.
struct Conversation;
impl maestro_tui::Component for Conversation {
    fn render(&self, width: usize) -> Vec<String> {
        vec![".".repeat(width); ROWS]
    }
}

/// The application's overlay presentation: a component with its own width and no placement
/// is shown that wide, focused, until it closes.
#[derive(Default)]
struct Ui {
    /// The frame writer the overlay is shown on.
    tui: RefCell<Option<TUI>>,
    /// The rows the demo renders before the frame writer focuses it.
    unfocused: RefCell<Vec<String>>,
    /// Tells the test the overlay is shown.
    shown: RefCell<Option<oneshot::Sender<()>>>,
    /// Notifications in arrival order.
    notes: RefCell<Vec<(String, Option<NotificationType>)>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        dark()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        // Taken first, so a failure below ends the test's wait instead of stalling it.
        let notify_shown = self.shown.take().unwrap();
        let options = options.unwrap();
        assert_eq!(options.overlay, Some(true));
        assert!(options.overlay_options.is_none() && options.on_handle.is_none());
        let (send, receive) = oneshot::channel();
        let send = RefCell::new(Some(send));
        let close =
            Rc::new(move || Ok(send.borrow_mut().take().is_some_and(|s| s.send(()).is_ok())));
        let keys = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let tui = self.tui.borrow().clone().unwrap();
        let Ok(CustomFactoryOutcome::Ready(built)) =
            factory.call((tui.clone(), self.theme(), keys, close))
        else {
            panic!("the overlay is built at once");
        };
        assert_eq!(built.width, Some(70.0));
        self.unfocused
            .replace(built.component.try_render(COLUMNS).unwrap());
        let placement = OverlayOptions {
            width: Some(SizeValue::Cells(70)),
            ..OverlayOptions::default()
        };
        let mut shown = tui
            .show_overlay(built.component, Some(Rc::new(RefCell::new(placement))))
            .unwrap();
        notify_shown.send(()).unwrap();
        Box::pin(async move {
            receive
                .await
                .map_err(|error| EventError::from(error.to_string()))?;
            shown.hide().unwrap();
            Ok(())
        })
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.notes.borrow_mut().push((message, kind));
        Ok(())
    }
}

/// The application's shipped dark theme.
fn dark() -> maestro_theme::LiveTheme {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-theme/assets/theme/dark.json"
    );
    let theme =
        maestro_theme::load_theme_from_path(path, Some(maestro_theme::ColorMode::Truecolor));
    maestro_theme::LiveTheme::from_theme(Rc::new(theme.unwrap()))
}

/// The built example on the production runner, over a frame writer showing a conversation.
struct Demo {
    /// Production runner over the compiled shipped example.
    runner: ExtensionRunner<Local>,
    /// The overlay host.
    ui: Rc<Ui>,
    /// The frame writer.
    tui: TUI,
    /// The terminal input arrives on and frames are written to.
    terminal: RecordingTerminal,
}
impl Demo {
    /// Runs `/overlay-test` until the overlay is shown, returning the running command.
    async fn open(&self) -> tokio::task::JoinHandle<Result<(), EventError>> {
        let (send, shown) = oneshot::channel();
        self.ui.shown.replace(Some(send));
        let handler = self
            .runner
            .get_command("overlay-test")
            .unwrap()
            .command
            .handler;
        let running =
            tokio::task::spawn_local(handler(String::new(), self.runner.create_command_context()));
        shown.await.unwrap();
        running
    }

    /// Delivers each input to the frame writer, which routes it to the focused overlay.
    fn keys(&self, inputs: &[&str]) {
        for input in inputs {
            self.terminal.send_input(input);
        }
    }

    /// Redraws the whole screen, returning its rows as shown and the cursor movement after.
    fn frame(&self) -> (Vec<String>, String) {
        self.terminal.clear_writes();
        self.tui.request_render(true);
        self.tui.render_pending_frame().unwrap();
        let output = self.terminal.writes().concat();
        let (_, frame) = output.split_once("\x1b[?2026h").unwrap();
        let (frame, cursor) = frame.split_once("\x1b[?2026l").unwrap();
        (frame.split("\r\n").map(plain).collect(), cursor.to_owned())
    }

    /// Runs the command with `inputs`, returning the notifications it sent.
    async fn choose(&self, inputs: &[&str]) -> Vec<(String, Option<NotificationType>)> {
        let running = self.open().await;
        self.keys(inputs);
        running.await.unwrap().unwrap();
        assert!(!self.tui.has_overlay());
        self.ui.notes.take()
    }
}

/// The text a terminal shows for `line`, without its escape sequences.
fn plain(line: &str) -> String {
    let mut shown = String::new();
    let mut index = 0;
    while let Some(character) = line[index..].chars().next() {
        if let Some(code) = extract_ansi_code(line, index) {
            index += code.length;
        } else {
            shown.push(character);
            index += character.len_utf8();
        }
    }
    shown
}

/// Builds the example and runs `body` with it loaded on a started frame writer.
fn run(body: impl AsyncFnOnce(&Demo)) {
    let path = super::extension_guide::cargo::component(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-extensions-wasm"
        )),
        &["--example", "overlay_test"],
        "overlay_test",
    )
    .unwrap();
    let path = path.to_str().unwrap().to_owned();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let cwd = env!("CARGO_MANIFEST_DIR");
        let extensions = create_extension_runtime::<Local>();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            cwd.into(),
            bus.for_extension(),
            extensions.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            extensions,
            super::session::runner_session(cwd).0,
            Rc::new(|_| {}),
            dark(),
        );
        let _remove = runner.on_error(Rc::new(|error| {
            panic!("unexpected callback error: {error:?}")
        }));
        let (tui, terminal) = screen();
        let ui = Rc::new(Ui::default());
        ui.tui.replace(Some(tui.clone()));
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let demo = Demo {
            runner,
            ui,
            tui,
            terminal,
        };
        body(&demo).await;
        demo.tui.stop().unwrap();
        bus.clear();
    });
}

/// A started frame writer showing the conversation on an 80 by 24 terminal.
fn screen() -> (TUI, RecordingTerminal) {
    let terminal = RecordingTerminal::new(COLUMNS, ROWS);
    let tui = TUI::new(
        terminal.handle(),
        ManualRuntime::new().handle(),
        TerminalImage::default(),
        Some(false),
    );
    tui.add_child(Rc::new(Conversation));
    tui.start().unwrap();
    (tui, terminal)
}

/// One overlay row: the content between borders, padded to 68 cells.
fn boxed(content: &str) -> String {
    let padding = 68 - maestro_tui::visible_width(content);
    format!("│{content}{}│", " ".repeat(padding))
}

/// The overlay's rows as shown, with `entries` in its menu.
fn overlay(entries: [&str; 4]) -> Vec<String> {
    let mut rows = vec![format!("╭{}╮", "─".repeat(68))];
    rows.extend(
        [
            " 🎼 Overlay demo",
            "",
            " ─── Width checks: borders stay aligned ───",
            " Wide: 幅広い文字で枠線の位置を確かめる한글도함께점검中文繁體简体字",
            " Colors: ERROR SUCCESS WARNING ACCENT DIM and again",
            " Emoji: 🧑\u{200d}🎤 🇨🇦 🎵 🎶 🎹 🎻 🎺 🥁 🎼 🎷 🪕 🎸 🎤 🎧 🎨 🎭 🎬 🎯 🎲 🌟",
            "",
            " ─── Menu ───",
        ]
        .into_iter()
        .chain(entries)
        .chain(["", " ↑↓ move • type to edit • Enter choose • Esc close"])
        .map(boxed),
    );
    rows.push(format!("╰{}╯", "─".repeat(68)));
    rows
}

/// The whole screen with `overlay` composed over the conversation's middle.
fn composed(overlay: &[String]) -> Vec<String> {
    let dots = ".".repeat(COLUMNS);
    let mut screen = vec![dots; ROWS];
    for (row, line) in overlay.iter().enumerate() {
        screen[4 + row] = format!(".....{line}.....");
    }
    screen
}

/// The cursor movement that puts the hardware cursor on overlay row `row`, `column` cells
/// right of the overlay's left border.
fn cursor(row: usize, column: usize) -> String {
    format!(
        "\x1b[{}A\x1b[{}G\x1b[?25l",
        ROWS - 1 - (4 + row),
        5 + column + 1
    )
}

#[test]
fn overlay_test_composes_aligned_rows_and_focuses_the_first_field() {
    run(async |demo| {
        let running = demo.open().await;
        let rows = overlay([" ▶ Find:  ", "   Launch: ", "   Preferences", "   Dismiss"]);
        let unfocused = demo.ui.unfocused.borrow().clone();
        assert!(!unfocused.concat().contains(maestro_tui::CURSOR_MARKER));
        assert_eq!(
            unfocused.iter().map(|row| plain(row)).collect::<Vec<_>>(),
            rows
        );
        for row in &rows {
            assert_eq!(maestro_tui::visible_width(row), 70, "{row}");
        }
        let (screen, moved) = demo.frame();
        assert_eq!(screen, composed(&rows));
        assert_eq!(moved, cursor(9, 10));
        demo.keys(&["\x1b"]);
        running.await.unwrap().unwrap();
        assert!(!demo.tui.has_overlay());
        assert!(demo.ui.notes.borrow().is_empty());
    });
}

#[test]
fn overlay_test_fields_edit_at_the_cursor_across_wide_characters() {
    run(async |demo| {
        let running = demo.open().await;
        demo.keys(&["a", "中", "c", LEFT, "\x7f", "\x01", RIGHT, RIGHT, LEFT]);
        demo.keys(&["😀"]);
        let (screen, moved) = demo.frame();
        let rows = overlay([
            " ▶ Find: a😀c",
            "   Launch: ",
            "   Preferences",
            "   Dismiss",
        ]);
        assert_eq!(screen, composed(&rows));
        assert_eq!(moved, cursor(9, 13));
        demo.keys(&["\r"]);
        running.await.unwrap().unwrap();
        assert_eq!(
            demo.ui.notes.take(),
            [("Find: \"a😀c\"".to_owned(), Some(NotificationType::Info))]
        );
    });
}

#[test]
fn overlay_test_menu_clamps_and_reports_each_choice() {
    run(async |demo| {
        let running = demo.open().await;
        demo.keys(&[UP, DOWN, DOWN, DOWN, DOWN, DOWN]);
        let (screen, moved) = demo.frame();
        let rows = overlay(["   Find: ", "   Launch: ", "   Preferences", " ▶ Dismiss"]);
        assert_eq!(screen, composed(&rows));
        assert_eq!(moved, "\x1b[?25l");
        demo.keys(&["\r"]);
        running.await.unwrap().unwrap();
        let info = Some(NotificationType::Info);
        assert_eq!(demo.ui.notes.take(), [("Dismiss".to_owned(), info)]);
        for (inputs, report) in [
            (&[DOWN, "x", "y", "\r"][..], Some("Launch: \"xy\"")),
            (&["\r"], Some("Find")),
            (&[DOWN, DOWN, "z", UP, "\r"], Some("Launch")),
            (&[DOWN, "x", "\x1b"], None),
        ] {
            let expected: Vec<_> = report
                .map(|report| (report.to_owned(), info))
                .into_iter()
                .collect();
            assert_eq!(demo.choose(inputs).await, expected, "{inputs:?}");
        }
    });
}

#[test]
fn overlay_test_joined_emoji_fields_stay_inside_the_border() {
    run(async |demo| {
        let running = demo.open().await;
        let a = |count| "a".repeat(count);
        demo.keys(&["a"; 58]);
        demo.keys(&["©", "\u{fe0f}", LEFT]);
        let find = format!(" ▶ Find: {}©\u{fe0f}", a(57));
        let rows = overlay([&find, "   Launch: ", "   Preferences", "   Dismiss"]);
        assert_eq!(demo.frame(), (composed(&rows), cursor(9, 68)));
        demo.keys(&[DOWN]);
        demo.keys(&["a"; 60]);
        demo.keys(&["©", "\u{fe0f}"]);
        demo.keys(&["a"; 60]);
        demo.keys(&[LEFT; 62]);
        let find = format!("   Find: {}©\u{fe0f}", a(57));
        let launch = format!(" ▶ Launch: {}©\u{fe0f}{}", a(28), a(27));
        let rows = overlay([&find, &launch, "   Preferences", "   Dismiss"]);
        assert_eq!(demo.frame(), (composed(&rows), cursor(10, 40)));
        demo.keys(&["\r"]);
        running.await.unwrap().unwrap();
        let report = format!("Launch: \"{}©\u{fe0f}{}\"", a(60), a(60));
        assert_eq!(
            demo.ui.notes.take(),
            [(report, Some(NotificationType::Info))]
        );
    });
}

#[test]
fn overlay_test_long_fields_scroll_inside_the_border() {
    run(async |demo| {
        let running = demo.open().await;
        let digits = "0123456789".repeat(10);
        let typed: Vec<String> = digits.chars().map(String::from).collect();
        demo.keys(&typed.iter().map(String::as_str).collect::<Vec<_>>());
        for (lefts, shown, column) in [(0, 42..100, 68), (50, 21..80, 39), (50, 0..59, 10)] {
            demo.keys(&vec![LEFT; lefts]);
            let find = format!(" ▶ Find: {}", &digits[shown]);
            let rows = overlay([&find, "   Launch: ", "   Preferences", "   Dismiss"]);
            assert_eq!(demo.frame(), (composed(&rows), cursor(9, column)));
        }
        demo.keys(&[DOWN]);
        demo.keys(&["中"; 40]);
        let find = format!("   Find: {}", &digits[..59]);
        let launch = format!(" ▶ Launch: {}", "中".repeat(28));
        let rows = overlay([&find, &launch, "   Preferences", "   Dismiss"]);
        for (lefts, column) in [(0, 68), (1, 66)] {
            demo.keys(&vec![LEFT; lefts]);
            assert_eq!(demo.frame(), (composed(&rows), cursor(10, column)));
        }
        demo.keys(&[UP, "\r"]);
        running.await.unwrap().unwrap();
        let report = format!("Find: \"{digits}\"");
        assert_eq!(
            demo.ui.notes.take(),
            [(report, Some(NotificationType::Info))]
        );
    });
}
