//! The overlay demonstrations through the shipped component, the production runner and a real
//! frame writer on a recording terminal.
#![cfg(test)]
use maestro_extensions::native_io::{
    ChildProcess, ExitStatus, FsWatcher, IoFuture, NativeIo, Readable, Request, Response,
    SpawnOptions, Writable,
};
use maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionComponent,
        ExtensionUIContextPort, ExtensionUIFuture, NotificationType, OverlayOptionsSource,
    },
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock, time::Duration};
use tokio::sync::{Mutex, mpsc};

/// One pipe the test writes; dropping its sender ends the stream.
struct Pipe(Mutex<mpsc::UnboundedReceiver<Vec<u8>>>);
impl Readable for Pipe {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async { Ok(self.0.lock().await.recv().await) })
    }
    fn close(&self) -> Result<(), String> {
        Ok(())
    }
}

/// A launched process whose output the test supplies and whose signals it records.
struct Child {
    /// Standard output.
    stdout: Rc<Pipe>,
    /// Standard error.
    stderr: Rc<Pipe>,
    /// Signals received, shared with the test.
    kills: Rc<RefCell<Vec<Option<String>>>>,
}
impl ChildProcess for Child {
    fn pid(&self) -> Option<u32> {
        Some(1)
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        None
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        Some(self.stdout.clone())
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        Some(self.stderr.clone())
    }
    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        Box::pin(std::future::ready(Ok(ExitStatus {
            code: Some(0),
            signal: None,
        })))
    }
    fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.kills.borrow_mut().push(signal);
        Ok(true)
    }
}

/// Controlled native effects: one scripted process launch.
struct Process {
    /// The launched program and its operands.
    launched: RefCell<Vec<(String, Vec<String>)>>,
    /// The process handed to the first launch.
    child: RefCell<Option<Child>>,
    /// Signals the process received.
    kills: Rc<RefCell<Vec<Option<String>>>>,
}
impl Process {
    /// A process and the senders of its standard output and error.
    fn new() -> (Rc<Self>, [mpsc::UnboundedSender<Vec<u8>>; 2]) {
        let (out, stdout) = mpsc::unbounded_channel();
        let (err, stderr) = mpsc::unbounded_channel();
        let kills = Rc::default();
        let child = Child {
            stdout: Rc::new(Pipe(Mutex::new(stdout))),
            stderr: Rc::new(Pipe(Mutex::new(stderr))),
            kills: Rc::clone(&kills),
        };
        let process = Self {
            launched: RefCell::default(),
            child: RefCell::new(Some(child)),
            kills,
        };
        (Rc::new(process), [out, err])
    }
}
impl NativeIo for Process {
    fn save_file(&self, _: String, _: Vec<u8>, _: u32) -> IoFuture<'_, Option<String>> {
        Box::pin(std::future::ready(Err("no file saves".into())))
    }
    fn file_mode(&self, _: &str) -> Result<u32, String> {
        Err("no file modes".into())
    }
    fn set_file_mode(&self, _: &str, _: u32) -> Result<(), String> {
        Err("no permission changes".into())
    }
    fn create_dir(&self, _: &str, _: u32) -> Result<(), String> {
        Err("no directory creation".into())
    }
    fn watch(&self, _: String, _: Rc<dyn Fn()>) -> Result<Rc<dyn FsWatcher>, String> {
        Err("no watches".into())
    }
    fn fetch(&self, _: Request) -> IoFuture<'_, Rc<dyn Response>> {
        Box::pin(std::future::ready(Err("no requests".into())))
    }
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        _: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        self.launched.borrow_mut().push((command, args));
        let child = self.child.take().ok_or_else(|| "one launch".to_owned());
        Box::pin(std::future::ready(
            child.map(|child| Rc::new(child) as Rc<dyn ChildProcess>),
        ))
    }
    fn kill(&self, _: i32, _: Option<String>) -> Result<(), String> {
        Err("no process groups".into())
    }
}

/// One custom overlay the example asked for, with the placement it supplied.
struct Shown {
    /// The shared placement record.
    placement: Rc<RefCell<maestro_tui::OverlayOptions>>,
    /// The built component.
    component: maestro_extensions::ComponentHandle,
}

/// The shown overlay and its component, until the view closes.
type Open = Rc<
    RefCell<
        Option<(
            Rc<RefCell<Box<dyn maestro_tui::OverlayHandle>>>,
            Rc<ExtensionComponent>,
        )>,
    >,
>;

/// An interactive host that shows custom views the way the interactive mode does: build,
/// show with the supplied placement, hand over the handle; closing removes the overlay,
/// settles the view, then disposes the component.
struct Ui {
    /// The real frame writer.
    tui: maestro_tui::TUI,
    /// The published dark theme.
    theme: maestro_theme::LiveTheme,
    /// Custom overlays in the order they were shown.
    shown: RefCell<Vec<Shown>>,
    /// Notifications and editor changes in arrival order.
    calls: RefCell<Vec<Value>>,
}
impl Ui {
    /// Builds and shows one view, returning the handle callback's failure.
    fn show(
        &self,
        factory: &CustomComponentFactory,
        options: Option<CustomUIOptions>,
        (open, close): (Open, maestro_extensions::types::CustomClose),
    ) -> Result<(), EventError> {
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let CustomFactoryOutcome::Ready(built) =
            factory.call((self.tui.clone(), self.theme(), keys, close))?
        else {
            panic!("every demonstration builds at once");
        };
        let options = options.unwrap();
        assert_eq!(options.overlay, Some(true));
        let Some(OverlayOptionsSource::Static(placement)) = options.overlay_options else {
            panic!("every demonstration supplies its placement");
        };
        let shown = self
            .tui
            .show_overlay(built.component.clone(), Some(placement.clone()))?;
        let handle = Rc::new(RefCell::new(shown));
        let component = built.component.clone();
        open.replace(Some((handle.clone(), Rc::new(built))));
        if let Some(on_handle) = &options.on_handle {
            on_handle.call(handle)?;
        }
        self.shown.borrow_mut().push(Shown {
            placement,
            component,
        });
        Ok(())
    }
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        self.theme.clone()
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!([message, kind]));
        Ok(())
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!(["editor", text]));
        Ok(())
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (settle, settled) = tokio::sync::oneshot::channel();
        let settle = RefCell::new(Some(settle));
        let open = Open::default();
        let closing = open.clone();
        let close = Rc::new(move || {
            let Some(settle) = settle.borrow_mut().take() else {
                return Ok(false);
            };
            let Some((handle, built)) = closing.take() else {
                panic!("a demonstration closes after it is shown");
            };
            handle.borrow_mut().hide()?;
            let _ = settle.send(());
            built
                .dispose
                .as_ref()
                .map_or(Ok(()), |dispose| dispose.call(()))?;
            Ok(true)
        });
        let shown = self.show(&factory, options, (open, close));
        Box::pin(async move {
            shown?;
            settled
                .await
                .map_err(|error| EventError::from(error.to_string()))
        })
    }
}

/// The production runner over the compiled example and the interactive host it is bound to.
struct Host {
    /// Runs the registered commands.
    runner: ExtensionRunner<Local>,
    /// The bound interactive host.
    ui: Rc<Ui>,
    /// The terminal under the frame writer.
    terminal: super::recording_terminal::RecordingTerminal,
    /// Extension errors the runner reported.
    errors: Rc<RefCell<Vec<String>>>,
}

/// The production runner over the compiled example, with the controlled process as its native
/// effects when supplied.
async fn runner(io: Option<Rc<Process>>) -> ExtensionRunner<Local> {
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no bus errors")),
    );
    let path = std::env::var("MAESTRO_OVERLAY_QA_TESTS_COMPONENT").expect("nextest setup");
    let factory = match io {
        Some(io) => maestro_extensions_wasmtime::artifact_factory_with_io(
            path.clone(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "installed-component-host".into(),
                agent_dir: "/controlled-agent".into(),
            },
            io,
        ),
        None => crate::artifact(path.clone()),
    };
    let extension = bus.for_extension();
    let owner =
        load_extension_from_factory(factory, "/".into(), extension, runtime.clone(), Some(path))
            .await
            .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    super::support::bind_prompt(&runner, "overlay demonstrations");
    runner
}

/// Loads the example on a started frame writer of `columns` by `rows` with the dark theme.
async fn host((columns, rows): (usize, usize), io: Option<Rc<Process>>) -> Host {
    let terminal = super::recording_terminal::RecordingTerminal::new(columns, rows);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        super::manual_runtime::ManualRuntime::new().handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    );
    tui.start().unwrap();
    let theme = maestro_theme::load_theme_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-theme/assets/theme/dark.json"
        ),
        Some(maestro_theme::ColorMode::Truecolor),
    )
    .unwrap();
    let runner = runner(io).await;
    let errors = Rc::new(RefCell::new(Vec::new()));
    let reported = errors.clone();
    drop(runner.on_error(Rc::new(move |error| {
        reported.borrow_mut().push(error.error.clone());
        Ok(())
    })));
    let ui = Rc::new(Ui {
        tui,
        theme: maestro_theme::LiveTheme::from_theme(Rc::new(theme)),
        shown: RefCell::default(),
        calls: RefCell::default(),
    });
    runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    Host {
        runner,
        ui,
        terminal,
        errors,
    }
}

/// Runs `case` on a fresh host of `size` and checks that no extension error was reported.
fn run(size: (usize, usize), io: Option<Rc<Process>>, case: impl AsyncFnOnce(&Host)) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let host = host(size, io).await;
        case(&host).await;
        assert!(
            host.errors.borrow().is_empty(),
            "{:?}",
            host.errors.borrow()
        );
    });
}

/// Lets `duration` of paused time pass. The clock moves only while every task waits, so each
/// native wait the example armed before this call is due at its own time.
async fn elapse(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// Yields to the runtime until `ready` holds.
async fn until(mut ready: impl FnMut() -> bool) {
    while !ready() {
        tokio::task::yield_now().await;
    }
}

/// Text without control sequences.
fn plain(text: &str) -> String {
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            skip_sequence(&mut chars);
        } else {
            plain.push(c);
        }
    }
    plain
}

/// Consumes one CSI, OSC or APC sequence after its escape.
fn skip_sequence(chars: &mut std::str::Chars<'_>) {
    match chars.next() {
        Some('[') => drop(chars.find(|c| ('@'..='~').contains(c))),
        Some(']' | '_') => drop(chars.find(|c| *c == '\x07')),
        _ => {}
    }
}

/// Row and column of the first top-left box corner on screen.
fn corner(screen: &[String]) -> Option<(usize, usize)> {
    screen
        .iter()
        .enumerate()
        .find_map(|(row, line)| Some((row, line.chars().position(|c| c == '╭')?)))
}

impl Host {
    /// Starts a registered command on the local executor.
    fn command(&self, name: &str) -> tokio::task::JoinHandle<Result<(), EventError>> {
        let handler = self.runner.get_command(name).unwrap().command.handler;
        tokio::task::spawn_local(handler(String::new(), self.runner.create_command_context()))
    }
    /// Waits until the example has shown `count` custom overlays.
    async fn shown(&self, count: usize) {
        until(|| self.ui.shown.borrow().len() == count).await;
    }
    /// Delivers terminal input through the frame writer's focus.
    fn press(&self, data: &str) {
        self.terminal.send_input(data);
    }
    /// The whole terminal after a full redraw, without control sequences.
    fn screen(&self) -> Vec<String> {
        let written = Rc::new(RefCell::new(String::new()));
        let tapped = written.clone();
        self.terminal
            .set_tap(move |data| tapped.borrow_mut().push_str(data));
        self.ui.tui.request_render(true);
        self.ui.tui.render_pending_frame().unwrap();
        self.terminal.set_tap(|_| {});
        let written = written.borrow();
        let frame = written.rsplit("\x1b[?2026h").next().unwrap();
        let frame = frame.split("\x1b[?2026l").next().unwrap();
        frame.split("\r\n").map(plain).collect()
    }
    /// The `index`th custom overlay's rows at `width`, without control sequences.
    fn rows(&self, index: usize, width: usize) -> Vec<String> {
        let component = self.ui.shown.borrow()[index].component.clone();
        component
            .try_render(width)
            .unwrap()
            .iter()
            .map(|row| plain(row))
            .collect()
    }
    /// Whether the `index`th custom overlay has a row containing `text`.
    fn shows(&self, index: usize, width: usize, text: &str) -> bool {
        self.rows(index, width).iter().any(|row| row.contains(text))
    }
    /// Notifications and editor changes so far.
    fn calls(&self) -> Vec<Value> {
        self.ui.calls.borrow().clone()
    }
    /// Waits for the command to succeed.
    async fn finished(&self, command: tokio::task::JoinHandle<Result<(), EventError>>) {
        command.await.unwrap().unwrap();
        assert!(!self.ui.tui.has_overlay(), "closing removes every overlay");
    }
}

/// An informational notification as the host receives it.
fn info(message: &str) -> Value {
    json!([message, "info"])
}

#[test]
fn overlay_qa_tests_register_every_demonstration() {
    run((80, 24), None, async |host| {
        for (name, description) in [
            (
                "overlay-animation",
                "Animate an overlay at about 30 frames a second",
            ),
            (
                "overlay-anchors",
                "Show an overlay at each of the nine anchors",
            ),
            (
                "overlay-margins",
                "Place an overlay with margins and offsets",
            ),
            ("overlay-stack", "Stack three overlapping overlays"),
            (
                "overlay-overflow",
                "Stream long process output into a narrower overlay",
            ),
            ("overlay-edge", "Place an overlay against the right edge"),
            (
                "overlay-percent",
                "Place an overlay by row and column percentages",
            ),
            ("overlay-maxheight", "Cut an overlay at its maximum height"),
            (
                "overlay-sidepanel",
                "Show a side panel that hides below 100 columns",
            ),
            ("overlay-toggle", "Hide an overlay for one second with t"),
            (
                "overlay-passive",
                "Type beside a timer that never takes focus",
            ),
            (
                "overlay-focus",
                "Move focus between overlapping non-capturing panels",
            ),
            (
                "overlay-streaming",
                "Type into panels beside a simulated stream",
            ),
        ] {
            let command = host.runner.get_command(name).unwrap().command;
            assert_eq!(command.description.as_deref(), Some(description), "{name}");
        }
    });
}

#[test]
fn overlay_qa_tests_anchors_cycle_all_nine_positions_and_confirm() {
    run((80, 24), None, async |host| {
        let anchors = [
            ("top-left", (0, 0)),
            ("top-center", (0, 20)),
            ("top-right", (0, 40)),
            ("left-center", (7, 0)),
            ("center", (7, 20)),
            ("right-center", (7, 40)),
            ("bottom-left", (15, 0)),
            ("bottom-center", (15, 20)),
            ("bottom-right", (15, 40)),
        ];
        let command = host.command("overlay-anchors");
        for (index, (name, position)) in anchors.into_iter().enumerate() {
            host.shown(index + 1).await;
            let screen = host.screen();
            assert_eq!(corner(&screen), Some(position), "{name}");
            assert!(screen[position.0 + 2].contains(&format!("│ Anchor: {name} ")));
            host.press(match index {
                8 => "\r",
                _ if index % 2 == 0 => " ",
                _ => "\x1b[C",
            });
        }
        host.finished(command).await;
        assert_eq!(host.calls(), [info("Anchor selected: bottom-right")]);
        let command = host.command("overlay-anchors");
        for index in 9..=18 {
            host.shown(index + 1).await;
            host.press(if index == 18 { "\x1b" } else { " " });
        }
        assert!(host.rows(18, 40)[2].contains("Anchor: top-left"));
        host.finished(command).await;
        assert_eq!(host.calls().len(), 1, "cancelling reports nothing");
    });
}

/// Cycles a configuration command once around, checking each corner and the wrap.
async fn cycle(host: &Host, name: &str, corners: [(usize, usize); 5], labels: [&str; 5]) {
    let command = host.command(name);
    for (index, (position, label)) in corners.into_iter().zip(labels).enumerate() {
        host.shown(index + 1).await;
        let screen = host.screen();
        assert_eq!(corner(&screen), Some(position), "{label}");
        assert!(screen[position.0 + 2].contains(label), "{label}");
        host.press(if index % 2 == 0 { " " } else { "\x1b[C" });
    }
    host.shown(6).await;
    assert_eq!(corner(&host.screen()), Some(corners[0]), "the cycle wraps");
    host.press("\x1b");
    host.finished(command).await;
    assert!(host.calls().is_empty());
}

#[test]
fn overlay_qa_tests_margins_and_offsets_place_each_configuration() {
    run((80, 24), None, async |host| {
        let corners = [(0, 0), (3, 3), (5, 10), (5, 32), (14, 43)];
        let labels = [
            "Top left, no margin",
            "Top left, margin 3",
            "Top left, margin top 5, left 10",
            "Center, 10 right and 3 up",
            "Bottom right, margin 2",
        ];
        cycle(host, "overlay-margins", corners, labels).await;
        let width = host.ui.shown.borrow()[0].placement.borrow().width.clone();
        assert_eq!(
            width,
            Some(maestro_extensions::widgets::SizeValue::Cells(35))
        );
    });
}

#[test]
fn overlay_qa_tests_percentages_place_rows_and_columns() {
    run((80, 24), None, async |host| {
        let corners = [(0, 25), (8, 25), (16, 25), (8, 0), (8, 50)];
        let labels = [
            "Row 0% (top)",
            "Row 50% (middle)",
            "Row 100% (bottom)",
            "Column 0% (left)",
            "Column 100% (right)",
        ];
        cycle(host, "overlay-percent", corners, labels).await;
    });
}

#[test]
fn overlay_qa_tests_stack_shows_layers_in_turn_and_reports_close_order() {
    run((80, 24), None, async |host| {
        tokio::time::pause();
        let command = host.command("overlay-stack");
        host.shown(1).await;
        assert_eq!(host.calls(), [info("Layer 1 of 3 (back) shown")]);
        for count in [2, 3] {
            elapse(Duration::from_millis(399)).await;
            assert_eq!(
                host.ui.shown.borrow().len(),
                count - 1,
                "each layer waits 400 ms"
            );
            elapse(Duration::from_millis(1)).await;
            host.shown(count).await;
        }
        let screen = host.screen();
        for (row, col) in [(2, 7), (6, 15), (10, 23)] {
            assert_eq!(screen[row].chars().nth(col), Some('╭'), "{row}, {col}");
        }
        assert!(screen[11].contains("│ Layer #3"));
        assert!(screen[12].contains("Depth: front, blue border"));
        for key in ["\x1b", "\r", "\x03"] {
            host.press(key);
        }
        host.finished(command).await;
        assert_eq!(
            host.calls(),
            [
                info("Layer 1 of 3 (back) shown"),
                info("Layer 2 of 3 (middle) shown"),
                info("Layer 3 of 3 (front) shown"),
                info("Close order: layer 3, layer 2, layer 1"),
            ]
        );
    });
}

#[test]
fn overlay_qa_tests_animation_advances_with_each_native_wait() {
    run((80, 24), None, async |host| {
        tokio::time::pause();
        let command = host.command("overlay-animation");
        host.shown(1).await;
        assert_eq!(corner(&host.screen()), Some((4, 15)));
        let bar = |host: &Host| host.rows(0, 50)[6].chars().position(|c| c == '█');
        assert!(host.shows(0, 50, "│ Frame: 0 "));
        assert!(host.shows(0, 50, "│ FPS: "));
        assert!(host.shows(0, 50, "│ Spinner: ◐ "));
        assert_eq!(bar(host), Some(19));
        elapse(Duration::from_millis(34)).await;
        until(|| host.shows(0, 50, "│ Frame: 1 ")).await;
        assert!(host.shows(0, 50, "│ Spinner: ◓ "));
        assert_eq!(bar(host), Some(20));
        assert!(host.shows(0, 50, "│ Each frame comes from one native wait."));
        host.press("\x1b");
        host.finished(command).await;
    });
}

/// The line count in the output view's title.
fn received(title: &str) -> usize {
    let count = title.split_once('(').map_or("", |(_, rest)| rest);
    let digits: String = count.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

#[test]
fn overlay_qa_tests_overflow_streams_process_output_within_the_width() {
    let (process, [out, err]) = Process::new();
    run((80, 24), Some(process.clone()), async |host| {
        let command = host.command("overlay-overflow");
        host.shown(1).await;
        until(|| !process.launched.borrow().is_empty()).await;
        let (program, args) = process.launched.borrow()[0].clone();
        assert_eq!((program.as_str(), args[0].as_str()), ("bash", "-c"));
        assert!(args[1].contains("for i in $(seq 1 100); do") && args[1].contains("sleep 0.3"));
        let link = "\x1b]8;;file:///long/path\x07▶ read: /Users/example/Documents/development/maestro/crates/maestro-chat/src/interactive_mode/components/very-long-component-name-that-exceeds-width-1.rs\x1b]8;;\x07";
        out.send(format!("Starting\nThis simulates\n\n{link}\n").into_bytes())
            .unwrap();
        until(|| received(&host.rows(0, 80)[0]) == 3).await;
        let screen = host.screen();
        assert_eq!(
            corner(&screen),
            Some((2, 0)),
            "the width is clamped to the terminal"
        );
        assert!(screen.iter().all(|row| row.chars().count() <= 80));
        assert!(
            screen[3].trim_matches(['│', ' ']).is_empty(),
            "no scroll position below a full view"
        );
        assert!(screen[6].starts_with("│ ▶ read: /Users/example/") && screen[6].ends_with("...│"));
        assert!(screen[19].contains("│ ● Running | ↑↓ scroll | Esc close"));
        let lines: Vec<_> = (4..=23).map(|line| format!("line {line}\n")).collect();
        out.send(lines.concat().into_bytes()).unwrap();
        until(|| received(&host.rows(0, 80)[0]) == 23).await;
        assert!(
            host.screen()[3].contains("│ ↑8 | ↓0"),
            "new output scrolls to the end"
        );
        host.press("\x1b[A");
        assert!(host.screen()[3].contains("│ ↑7 | ↓1"));
        host.press("\x1b[B");
        host.press("\x1b[B");
        assert!(
            host.screen()[3].contains("│ ↑8 | ↓0"),
            "scrolling stops at the end"
        );
        err.send(b"  failed  \n".to_vec()).unwrap();
        until(|| received(&host.rows(0, 80)[0]) == 24).await;
        assert!(
            host.screen()[3].contains("│ ↑8 | ↓1"),
            "errors do not scroll"
        );
        drop((out, err));
        until(|| host.shows(0, 80, "│ ✓ Exited | ↑↓ scroll | Esc close")).await;
        assert!(
            host.screen()[3].contains("│ ↑8 | ↓1"),
            "closing the pipes keeps the scroll position"
        );
        host.press("\x1b");
        host.finished(command).await;
        assert_eq!(
            *process.kills.borrow(),
            [None, None],
            "Escape and disposal both stop it"
        );
    });
}

#[test]
fn overlay_qa_tests_overflow_joins_lines_split_across_reads() {
    let (process, [out, err]) = Process::new();
    run((80, 24), Some(process), async |host| {
        let command = host.command("overlay-overflow");
        host.shown(1).await;
        let deseret = "\u{10400}".as_bytes();
        let reads = [
            (&out, b"ab".to_vec()),
            (&out, b"cd\n".to_vec()),
            (&err, b"fa".to_vec()),
            (&err, b"iled\n".to_vec()),
            (&out, deseret[..2].to_vec()),
            (&out, [&deseret[2..], b" tail"].concat()),
        ];
        for (pipe, bytes) in reads {
            pipe.send(bytes).unwrap();
        }
        drop((out, err));
        until(|| host.shows(0, 80, "│ ✓ Exited | ↑↓ scroll | Esc close")).await;
        let rows = host.rows(0, 80);
        assert_eq!(received(&rows[0]), 3, "{rows:#?}");
        for line in ["abcd", "failed", "\u{10400} tail"] {
            let shown = format!("│ {line} ");
            assert!(rows.iter().any(|row| row.starts_with(&shown)), "{rows:#?}");
        }
        host.press("\x1b");
        host.finished(command).await;
    });
}

#[test]
fn overlay_qa_tests_edge_and_max_height_place_their_notices() {
    run((80, 24), None, async |host| {
        let command = host.command("overlay-edge");
        host.shown(1).await;
        let screen = host.screen();
        assert_eq!(corner(&screen), Some((6, 40)));
        assert_eq!(
            screen[6].chars().nth(79),
            Some('╮'),
            "the border meets the edge"
        );
        host.press("\x1b");
        host.finished(command).await;
        let command = host.command("overlay-maxheight");
        host.shown(2).await;
        let screen = host.screen();
        assert_eq!(corner(&screen), Some((7, 15)));
        assert!(screen[7].contains(" Maximum height "));
        assert!(screen[16].contains("│ Row 6 of 14"));
        assert!(screen[17].trim().is_empty());
        assert!(
            !screen.iter().any(|row| row.contains('╰')),
            "the bottom border is cut"
        );
        host.press("\x03");
        host.finished(command).await;
    });
}

#[test]
fn overlay_qa_tests_sidepanel_hides_below_one_hundred_columns() {
    run((120, 30), None, async |host| {
        let command = host.command("overlay-sidepanel");
        host.shown(1).await;
        let screen = host.screen();
        assert_eq!(corner(&screen), Some((8, 89)));
        assert!(screen[11].contains("│→ Sessions"));
        host.press("\x1b[B");
        assert!(host.screen()[12].contains("│→ Models"));
        for key in ["\x1b[A", "\x1b[A", "\r"] {
            host.press(key);
        }
        assert!(host.screen()[11].contains("│→ Sessions"));
        host.terminal.resize(99, 30);
        assert!(!host.ui.tui.has_overlay());
        assert_eq!(corner(&host.screen()), None);
        host.terminal.resize(100, 30);
        assert!(host.ui.tui.has_overlay());
        host.press("\x1b");
        host.finished(command).await;
    });
}

#[test]
fn overlay_qa_tests_toggle_hides_for_one_second_through_its_handle() {
    run((80, 24), None, async |host| {
        tokio::time::pause();
        let command = host.command("overlay-toggle");
        host.shown(1).await;
        assert_eq!(corner(&host.screen()), Some((3, 15)));
        for count in 1..=2 {
            host.press("t");
            assert!(!host.ui.tui.has_overlay());
            host.press("t");
            assert!(host.shows(0, 50, &format!("│ Times hidden: {count} ")));
            elapse(Duration::from_millis(999)).await;
            assert!(!host.ui.tui.has_overlay(), "hidden for one second");
            elapse(Duration::from_millis(1)).await;
            until(|| host.ui.tui.has_overlay()).await;
        }
        host.press("\x1b");
        host.finished(command).await;
    });
}

#[test]
fn overlay_qa_tests_passive_timer_leaves_typing_with_the_controller() {
    run((80, 24), None, async |host| {
        tokio::time::pause();
        let command = host.command("overlay-passive");
        host.shown(1).await;
        assert_eq!(host.calls(), [json!(["editor", ""])]);
        let screen = host.screen();
        assert_eq!(corner(&screen), Some((1, 56)), "the timer is top-right");
        assert!(screen[2].contains("│ 00:00"));
        assert!(screen[6].contains(" Typing "));
        assert!(screen[8].contains("│ Focused: true, inputs: 0"));
        assert!(screen[9].contains("│ Last input: none"));
        for key in ["h", "i", "\x7f"] {
            host.press(key);
        }
        let screen = host.screen();
        assert!(screen[8].contains("│ Focused: true, inputs: 3"));
        assert!(screen[9].contains("│ Last input: length 1, code 127"));
        assert!(screen[11].contains("│ > h "));
        host.press("\u{10400}");
        assert!(host.screen()[11].contains("│ > h\u{10400} "));
        host.press("\x7f");
        assert!(host.screen()[11].contains("│ > h "));
        elapse(Duration::from_secs(1)).await;
        until(|| host.screen()[2].contains("│ 00:01")).await;
        host.press("\x1b");
        host.finished(command).await;
    });
}

#[test]
fn overlay_qa_tests_focus_cycles_panels_and_raises_the_focused_one() {
    run((80, 24), None, async |host| {
        let command = host.command("overlay-focus");
        host.shown(1).await;
        let screen = host.screen();
        assert!(screen[3].contains("│ Firs│ Seco│ Third"), "{}", screen[3]);
        assert!(screen[11].contains("Focus: Controller"));
        host.press("\t");
        let screen = host.screen();
        assert!(
            screen[3].contains("│ First "),
            "the focused panel renders on top"
        );
        assert!(screen[5].contains("│ ● Focused"));
        assert!(screen[11].contains("Focus: First"));
        for (label, col) in [("Second", 13), ("Third", 19)] {
            host.press("\t");
            let screen = host.screen();
            assert_eq!(
                screen[3]
                    .chars()
                    .skip(col)
                    .take(label.len())
                    .collect::<String>(),
                label
            );
            assert!(screen[11].contains(&format!("Focus: {label}")));
        }
        host.press("\t");
        assert!(host.screen()[11].contains("Focus: Controller"));
        host.press("\x1b");
        host.finished(command).await;
    });
}

/// Whether a stream row is `[<local time>] Line <count>`.
fn streamed(row: &str, count: u64) -> bool {
    let Some((time, rest)) = row.trim_start_matches(['│', ' ']).split_once("] ") else {
        return false;
    };
    let time = time.trim_start_matches('[');
    rest.trim_end_matches(['│', ' ']) == format!("Line {count}")
        && (time.ends_with(" AM") || time.ends_with(" PM"))
        && time.matches(':').count() == 2
}

#[test]
fn overlay_qa_tests_streaming_routes_typing_to_the_focused_panel() {
    run((80, 24), None, async |host| {
        tokio::time::pause();
        let command = host.command("overlay-streaming");
        host.shown(1).await;
        assert_eq!(host.calls(), [json!(["editor", ""])]);
        for count in 1..=9 {
            elapse(Duration::from_millis(500)).await;
            until(|| host.rows(0, 60).iter().any(|row| streamed(row, count))).await;
        }
        let rows = host.rows(0, 60);
        assert_eq!(rows.iter().filter(|row| row.contains("] Line ")).count(), 8);
        assert!(
            !rows.iter().any(|row| streamed(row, 1)),
            "the oldest line scrolls away"
        );
        host.press("\t");
        for key in ["x", "z", "\x7f"] {
            host.press(key);
        }
        assert!(
            host.screen()[6].contains("│ > x "),
            "input A receives typing"
        );
        host.press("\u{10400}");
        assert!(host.screen()[6].contains("│ > x\u{10400} "));
        host.press("\x7f");
        assert!(host.screen()[6].contains("│ > x "));
        host.press("\t");
        host.press("y");
        assert!(
            host.screen()[15].contains("│ > y "),
            "input B receives typing"
        );
        assert!(host.shows(0, 60, "Focus: Input B"));
        host.press("\x1b");
        host.finished(command).await;
    });
}
