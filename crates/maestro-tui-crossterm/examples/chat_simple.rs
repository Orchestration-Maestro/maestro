//! A chat editor with local simulated replies and command completion.

#[cfg(unix)]
#[path = "test_themes/color.rs"]
mod color;
#[cfg(unix)]
#[path = "test_themes/default_editor_theme.rs"]
mod default_editor_theme;
#[cfg(unix)]
#[path = "test_themes/default_markdown_theme.rs"]
mod default_markdown_theme;
#[cfg(unix)]
#[path = "test_themes/default_select_list_theme.rs"]
mod default_select_list_theme;
#[path = "support/process_terminal.rs"]
mod process_terminal;

/// Native chat composition and retained reply state.
#[cfg(unix)]
mod unix {
    use super::color::{Color, color, decorate};
    use maestro_tui::{
        Editor, EditorOptions, Loader, Markdown, MarkdownOptions, MarkdownTheme, TUI,
        TerminalImage, Text,
        autocomplete::{
            CombinedAutocompleteProvider, Command, NativeAutocompleteOperations, SlashCommand,
        },
        keys::matches_key,
        text::utils::is_whitespace_char,
        tui::{ChildArray, ComponentHandle, InputListenerResult},
    };
    use std::{
        cell::{Cell, RefCell},
        io,
        process::ExitCode,
        rc::{Rc, Weak},
        time::Duration,
    };
    use tokio::{sync::Notify, task::JoinHandle};

    /// The editor's pending reply and the original child array it edits.
    struct Chat {
        /// Writer for this conversation.
        tui: TUI,
        /// Focused message input.
        editor: Rc<Editor>,
        /// Shared component entries, including welcome and editor.
        children: ChildArray,
        /// Shared Markdown styles.
        theme: Rc<MarkdownTheme>,
        /// Image state supplied to Markdown.
        images: TerminalImage,
        /// Spinner foreground.
        spinner: Color,
        /// Thinking message foreground.
        message: Color,
        /// Rejects repeated submissions while a reply is pending.
        responding: Cell<bool>,
        /// Loader retained until completion or shutdown.
        loader: RefCell<Option<Rc<Loader>>>,
        /// Pending native reply future.
        reply: RefCell<Option<JoinHandle<()>>>,
    }

    impl Chat {
        /// Builds command completion before the terminal starts.
        fn new(tui: TUI, images: TerminalImage) -> io::Result<Rc<Self>> {
            let editor = Rc::new(Editor::new(
                &tui,
                super::default_editor_theme::default_editor_theme()?,
                EditorOptions::default(),
            ));
            let commands = [
                ("delete", "Delete the last message"),
                ("clear", "Clear all messages"),
            ]
            .into_iter()
            .map(|(name, description)| {
                Command::SlashCommand(SlashCommand {
                    name: name.into(),
                    description: Some(description.into()),
                    argument_hint: None,
                    get_argument_completions: None,
                })
            })
            .collect();
            editor.set_autocomplete_provider(Rc::new(CombinedAutocompleteProvider::new(
                commands,
                std::env::current_dir()?.to_string_lossy().into_owned(),
                None,
                NativeAutocompleteOperations::default(),
            )));
            tui.add_child(Rc::new(Text::new("Welcome to Simple Chat!\n\nType your messages below. Type '/' for commands. Press Ctrl+C to exit.".into(), 0, 0, None)));
            tui.add_child(editor.clone());
            tui.set_focus(Some(editor.clone()));
            let message = color("text")?;
            let chat = Rc::new(Self {
                children: tui.children(),
                tui,
                editor,
                images,
                theme: Rc::new(super::default_markdown_theme::default_markdown_theme()?),
                spinner: color("live")?,
                message: Rc::new(move |text| decorate(&message(text), "\x1b[2m", "\x1b[22m")),
                responding: Cell::new(false),
                loader: RefCell::default(),
                reply: RefCell::default(),
            });
            let weak = Rc::downgrade(&chat);
            chat.editor.set_on_submit(Some(Rc::new(move |value| {
                Self::submit_weak(&weak, value);
            })));
            Ok(chat)
        }

        /// Invokes submission without retaining callback ownership.
        fn submit_weak(weak: &Weak<Self>, value: &str) {
            if let Some(chat) = weak.upgrade() {
                chat.submit(value);
            }
        }

        /// Completes a reply only while the conversation remains owned.
        fn complete_weak(weak: &Weak<Self>) {
            if let Some(chat) = weak.upgrade() {
                chat.complete();
            }
        }

        /// Removes the last message or the whole message region in place.
        fn edit_messages(&self, last: bool) {
            let mut children = self.children.borrow_mut();
            let end = children.len() - 1;
            let start = if last {
                end.saturating_sub(1).max(1)
            } else {
                1
            };
            let removed = children.drain(start..end).collect::<Vec<_>>();
            drop(children);
            drop(removed);
        }

        /// Inserts a message immediately before the editor.
        fn insert(&self, component: ComponentHandle) {
            let mut children = self.children.borrow_mut();
            let index = children.len() - 1;
            children.insert(index, component);
        }

        /// Constructs a transparent, padded Markdown message.
        fn markdown(&self, text: String) -> ComponentHandle {
            Rc::new(Markdown::new(
                text,
                MarkdownOptions {
                    padding_x: 1,
                    padding_y: 1,
                    default_text_style: None,
                },
                Rc::clone(&self.theme),
                self.images.clone(),
            ))
        }

        /// Handles commands or starts a single local reply.
        fn submit(self: &Rc<Self>, value: &str) {
            if self.responding.get() {
                return;
            }
            let trimmed = value
                .trim_matches(|scalar: char| is_whitespace_char(scalar.encode_utf8(&mut [0; 4])));
            if matches!(trimmed, "/delete" | "/clear") {
                self.edit_messages(trimmed == "/delete");
                self.tui.request_render(false);
                return;
            }
            if trimmed.is_empty() {
                return;
            }
            self.responding.set(true);
            self.editor.set_disable_submit(true);
            self.insert(self.markdown(value.into()));
            let loader = Rc::new(Loader::new(
                self.tui.clone(),
                Rc::clone(&self.spinner),
                Rc::clone(&self.message),
                Some("Thinking...".into()),
                None,
            ));
            self.insert(loader.clone());
            self.loader.replace(Some(loader));
            self.tui.request_render(false);
            let weak = Rc::downgrade(self);
            self.reply
                .replace(Some(tokio::task::spawn_local(async move {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    Self::complete_weak(&weak);
                })));
        }

        /// Stops the indicator and appends one uniformly selected reply.
        fn complete(&self) {
            self.stop_loader();
            let responses = [
                "That's interesting! Tell me more.",
                "I see what you mean.",
                "Fascinating perspective!",
                "Could you elaborate on that?",
                "That makes sense to me.",
                "I hadn't thought of it that way.",
                "Great point!",
                "Thanks for sharing that.",
            ];
            self.insert(self.markdown(responses[rand::random_range(0..responses.len())].into()));
            self.responding.set(false);
            self.editor.set_disable_submit(false);
            self.tui.request_render(false);
        }

        /// Stops and removes the retained loader before releasing it.
        fn stop_loader(&self) {
            let loader = self.loader.take();
            if let Some(loader) = loader {
                loader.stop();
                let component: ComponentHandle = loader;
                self.tui.remove_child(&component);
            }
        }

        /// Detaches submission and cancels pending reply work on exit.
        async fn close(&self) {
            self.editor.set_on_submit(None);
            let reply = self.reply.take();
            if let Some(reply) = reply {
                reply.abort();
                let _ = reply.await;
            }
            self.stop_loader();
        }
    }

    /// Consumes only the caller's cancellation key.
    fn quit_input(quit: &Notify, data: &str) -> InputListenerResult {
        if matches_key(data, "ctrl+c") {
            quit.notify_one();
            InputListenerResult::Consume
        } else {
            InputListenerResult::Pass
        }
    }

    /// Starts the editor and closes its reply state before native cleanup.
    pub(super) fn main() -> io::Result<ExitCode> {
        super::process_terminal::run(async |tui, images| {
            let chat = Chat::new(tui.clone(), images)?;
            let quit = Rc::new(Notify::new());
            let input_quit = Rc::clone(&quit);
            let remove = tui.add_input_listener(Rc::new(move |data| quit_input(&input_quit, data)));
            let result = async {
                tui.start()?;
                quit.notified().await;
                Ok(ExitCode::SUCCESS)
            }
            .await;
            remove();
            chat.close().await;
            result
        })
    }
}

/// Starts the native chat demonstration.
#[cfg(unix)]
fn main() -> std::io::Result<std::process::ExitCode> {
    unix::main()
}

/// Reports the pending native adapter on unsupported platforms.
#[cfg(not(unix))]
fn main() -> std::io::Result<std::process::ExitCode> {
    process_terminal::unsupported()
}
