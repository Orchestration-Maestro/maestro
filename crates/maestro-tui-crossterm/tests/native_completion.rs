#![cfg(test)]
//! A retained editor completing asynchronously on the native host.
#[allow(dead_code, reason = "Each test crate uses part of the shared helpers.")]
mod runtime_support;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::autocomplete::{
    AutocompleteOperations, CompletionError, CompletionOptions, CompletionResult, CursorPosition,
    NativeAutocompleteOperations,
};
use maestro_tui::tui::TerminalHandle;
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Component, Editor,
    EditorOptions, EditorTheme, KeybindingsManager, SelectListTheme, TUI, TUI_KEYBINDINGS,
    Terminal, TerminalImage, set_keybindings, tui::InputHandler,
};
use maestro_tui_crossterm::ProcessTuiRuntime;
use runtime_support::{is_child, rerun, run_set};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{advance, sleep, timeout};

/// What a provider future yields.
type Reply = Result<Option<AutocompleteSuggestions>, CompletionError>;

/// The cancellation signal the toolkit's own provider operations use.
type Signal = <NativeAutocompleteOperations as AutocompleteOperations>::Signal;

/// Waits for `future`; a paused clock that has nothing left to run elapses the limit at once,
/// so a request the host never ran fails the test instead of hanging it.
async fn within<T>(future: impl Future<Output = T>) -> T {
    timeout(Duration::from_secs(30), future)
        .await
        .expect("the awaited effect happened")
}

/// Lets every task that can run do so.
async fn settle() {
    sleep(Duration::from_millis(1)).await;
}

/// A provider that holds each reply until the test sends it.
struct Gate {
    /// Senders of the requests awaiting a reply, oldest first.
    held: RefCell<VecDeque<oneshot::Sender<Reply>>>,
    /// Announces the lines of each request when its future first runs.
    started: mpsc::UnboundedSender<Vec<String>>,
}

impl Gate {
    /// A provider and the stream of request announcements.
    fn new() -> (Rc<Self>, mpsc::UnboundedReceiver<Vec<String>>) {
        let (started, requests) = mpsc::unbounded_channel();
        let gate = Rc::new(Self {
            held: RefCell::new(VecDeque::new()),
            started,
        });
        (gate, requests)
    }

    /// Answers the oldest held request.
    fn reply(&self, reply: Reply) {
        let sender = self.held.borrow_mut().pop_front().expect("a held request");
        assert!(sender.send(reply).is_ok(), "the request is still waiting");
    }
}

impl AutocompleteProvider for Gate {
    type Signal = Signal;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        _cursor: CursorPosition,
        _options: CompletionOptions<'a, Signal>,
    ) -> Pin<Box<dyn Future<Output = Reply> + 'a>> {
        let (sender, receiver) = oneshot::channel();
        self.held.borrow_mut().push_back(sender);
        self.started.send(lines.to_vec()).expect("test listens");
        Box::pin(async move {
            receiver
                .await
                .unwrap_or_else(|_| Err("completion gate closed".into()))
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        let mut lines = lines.to_vec();
        let start = cursor.col - prefix.len();
        lines[cursor.line].replace_range(start..cursor.col, &item.value);
        CompletionResult {
            lines,
            cursor_line: cursor.line,
            cursor_col: start + item.value.len(),
        }
    }

    fn should_trigger_file_completion(&self, _: &[String], _: CursorPosition) -> Option<bool> {
        None
    }
}

/// A terminal that sends every write to the test.
struct Sink {
    /// Receives each written text.
    writes: mpsc::UnboundedSender<String>,
}

impl Terminal for Sink {
    fn start(&mut self, _: Box<dyn FnMut(&str)>, _: Box<dyn FnMut()>) -> io::Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn drain_input(
        &mut self,
        _: Option<Duration>,
        _: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.writes.send(data.to_owned()).expect("test listens");
        Ok(())
    }

    fn columns(&self) -> usize {
        80
    }

    fn rows(&self) -> usize {
        24
    }

    fn kitty_protocol_active(&self) -> bool {
        false
    }

    fn move_by(&mut self, _: isize) -> io::Result<()> {
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clear_line(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn set_title(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn set_progress(&mut self, _: bool) -> io::Result<()> {
        Ok(())
    }
}

/// Consumes terminal writes up to and including the first that contains `text`.
async fn read_until(writes: &mut mpsc::UnboundedReceiver<String>, text: &str) {
    while !writes
        .recv()
        .await
        .expect("the terminal stays open")
        .contains(text)
    {}
}

/// A started retained editor on the native host, with its provider and observations.
struct Scene {
    /// Keeps the writer, and with it the pending frame request, alive.
    _tui: TUI,
    /// The editor under test.
    editor: Rc<Editor>,
    /// The provider holding replies.
    gate: Rc<Gate>,
    /// Announcements of provider requests.
    requests: mpsc::UnboundedReceiver<Vec<String>>,
    /// Terminal writes in order.
    writes: mpsc::UnboundedReceiver<String>,
}

impl Scene {
    /// Starts a writer around a focused editor on the host of `local`.
    fn start(local: Rc<tokio::task::LocalSet>) -> Self {
        set_keybindings(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new()));
        let (writes_in, writes) = mpsc::unbounded_channel();
        let terminal: TerminalHandle = Rc::new(RefCell::new(Sink { writes: writes_in }));
        let tui = TUI::new(
            terminal,
            Rc::new(ProcessTuiRuntime::new(local)),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        let identity: Rc<dyn Fn(&str) -> String> = Rc::new(str::to_owned);
        let theme = EditorTheme {
            border_color: Rc::clone(&identity),
            select_list: SelectListTheme {
                selected_prefix: Rc::clone(&identity),
                selected_text: Rc::clone(&identity),
                description: Rc::clone(&identity),
                scroll_info: Rc::clone(&identity),
                no_match: identity,
            },
        };
        let editor = Rc::new(Editor::new(&tui, theme, EditorOptions::default()));
        tui.add_child(editor.clone());
        tui.set_focus(Some(editor.clone()));
        let (gate, requests) = Gate::new();
        editor.set_autocomplete_provider(gate.clone());
        tui.start().expect("writer starts");
        Self {
            _tui: tui,
            editor,
            gate,
            requests,
            writes,
        }
    }

    /// Types each character as its own input.
    fn typed(&self, text: &str) {
        for scalar in text.chars() {
            self.editor.handle_input(&scalar.to_string());
        }
    }

    /// Waits for the provider to receive a request and returns its lines.
    async fn request(&mut self) -> Vec<String> {
        within(self.requests.recv()).await.expect("a request")
    }

    /// Waits until the terminal receives a write that contains `text`.
    async fn drawn(&mut self, text: &str) {
        within(read_until(&mut self.writes, text)).await;
    }

    /// Whether the terminal has a write waiting that contains `text`.
    fn has_drawn(&mut self, text: &str) -> bool {
        std::iter::from_fn(|| self.writes.try_recv().ok()).any(|write| write.contains(text))
    }

    /// The menu rows below the editor, without padding.
    fn menu(&self) -> Vec<String> {
        let lines = self.editor.render(80);
        lines[self.editor.get_lines().len() + 2..]
            .iter()
            .map(|row| row.trim_end().to_owned())
            .collect()
    }
}

/// The suggestions of the given values for a prefix.
fn offer(prefix: &str, values: &[&str]) -> AutocompleteSuggestions {
    AutocompleteSuggestions {
        items: values
            .iter()
            .map(|value| AutocompleteItem {
                value: (*value).to_owned(),
                label: (*value).to_owned(),
                description: None,
            })
            .collect(),
        prefix: prefix.to_owned(),
    }
}

const TAB: &str = "\t";

#[test]
fn retained_editor_displays_async_native_suggestions() {
    run_set(true, |local| async move {
        let mut scene = Scene::start(local);
        scene.typed("src");
        scene.editor.handle_input(TAB);
        assert_eq!(scene.request().await, ["src"]);
        assert!(!scene.has_drawn("src.txt"));
        assert!(!scene.editor.is_showing_autocomplete());

        scene
            .gate
            .reply(Ok(Some(offer("src", &["src/", "src.txt"]))));
        scene.drawn("src.txt").await;
        assert!(scene.editor.is_showing_autocomplete());
        let rows = scene.menu();
        assert!(rows.iter().any(|row| row.ends_with("src/")), "{rows:?}");
        assert!(rows.iter().any(|row| row.ends_with("src.txt")), "{rows:?}");

        scene.editor.handle_input(TAB);
        assert_eq!(scene.editor.get_text(), "src/");
    });
}

#[test]
fn symbol_completion_debounces_on_native_host() {
    for (typed, prefix, value) in [("@mai", "@mai", "@main.ts"), ("#298", "#298", "#2983")] {
        run_set(true, |local| async move {
            let mut scene = Scene::start(local);
            scene.typed(typed);
            advance(Duration::from_millis(19)).await;
            settle().await;
            assert!(
                scene.requests.try_recv().is_err(),
                "{typed}: still debouncing"
            );
            advance(Duration::from_millis(1)).await;
            assert_eq!(scene.request().await, [typed]);
            settle().await;
            assert!(
                scene.requests.try_recv().is_err(),
                "{typed}: one request only"
            );
            scene.gate.reply(Ok(Some(offer(prefix, &[value]))));
            scene.drawn(value).await;
            assert!(
                scene.menu().iter().any(|row| row.ends_with(value)),
                "{typed}"
            );
        });
    }
}

#[test]
fn editor_error_is_reported_before_later_completion() {
    if is_child() {
        run_set(true, |local| async move {
            let mut scene = Scene::start(local);
            scene.typed("src");
            scene.editor.handle_input(TAB);
            scene.request().await;
            scene.gate.reply(Err("provider offline".into()));
            settle().await;
            assert!(!scene.editor.is_showing_autocomplete());

            scene.editor.handle_input(TAB);
            scene.request().await;
            scene
                .gate
                .reply(Ok(Some(offer("src", &["src/", "src.txt"]))));
            scene.drawn("src.txt").await;
            assert!(scene.editor.is_showing_autocomplete());
        });
        return;
    }
    let output = rerun("editor_error_is_reported_before_later_completion", |_| {});
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "provider offline\n"
    );
}
