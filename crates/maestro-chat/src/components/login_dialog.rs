//! Retained provider sign-in presentation.
use super::dynamic_border::DynamicBorder;
use maestro_theme::{LiveTheme, Theme, ThemeColor, ThemeError};
use maestro_tui::tui::InputHandler;
use maestro_tui::{Cancellation, Component, Container, FocusFlag, Focusable, Input, Spacer, Text};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use tokio::sync::oneshot;

/// Host observation used for click hints and native browser selection.
#[derive(Clone, Copy, Default)]
pub enum LoginDialogPlatform {
    /// macOS browser opener and command-click hint.
    #[cfg_attr(target_os = "macos", default)]
    MacOs,
    /// Windows literal-argument browser opener.
    #[cfg_attr(target_os = "windows", default)]
    Windows,
    /// Other hosts use the desktop opener.
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), default)]
    Other,
}
/// Replaceable provider-name observation.
type ProviderLookup = Rc<dyn Fn(&str) -> Option<String>>;
/// Replaceable browser launch effect.
type BrowserLauncher = Rc<dyn Fn(&str)>;
/// Caller completion notification.
type Completion = Rc<dyn Fn(bool, Option<&str>)>;
/// Constructor effects and title precedence.
pub struct LoginDialogOptions {
    /// Theme sampled when rows are added; borders remain live.
    pub theme: LiveTheme,
    /// Nonempty display name takes precedence over the registry.
    pub provider_name_override: Option<String>,
    /// Present titles, including empty titles, take precedence.
    pub title_override: Option<String>,
    /// Host observation for presentation and launch selection.
    pub platform: LoginDialogPlatform,
    /// Absent lookup uses the application's provider registry.
    pub provider_name_lookup: Option<ProviderLookup>,
    /// Browser embeddings supply their own launch effect.
    pub launch_browser: Option<BrowserLauncher>,
}
impl LoginDialogOptions {
    /// Consumes construction-only effects and captures the styled title.
    fn title(&mut self, provider_id: &str) -> Result<String, ThemeError> {
        let name = self.provider_name_lookup.take().map_or_else(
            || {
                maestro_app::presentation_data::get_oauth_providers()
                    .into_iter()
                    .find(|provider| provider.id() == provider_id)
                    .map(|provider| provider.name().to_owned())
            },
            |lookup| lookup(provider_id),
        );
        let name = self
            .provider_name_override
            .take()
            .filter(|name| !name.is_empty())
            .or_else(|| name.filter(|name| !name.is_empty()))
            .unwrap_or_else(|| provider_id.to_owned());
        let title = self.title_override.take();
        let title = title.unwrap_or_else(|| format!("Login to {name}"));
        self.theme
            .get()?
            .fg(&ThemeColor::Accent, &Theme::bold(&title, true))
    }
}
/// Cancellation is the only input-result failure.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginDialogError {
    /// The dialog or pending sender was closed.
    Cancelled,
}
impl std::fmt::Display for LoginDialogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Login cancelled")
    }
}
impl std::error::Error for LoginDialogError {}
/// Eagerly installed input receiver independent of the dialog's lifetime.
pub type LoginInputFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LoginDialogError>>>>;
/// Shared settlement state never retains the input or enclosing panel.
struct Settlement {
    /// Shared cooperative cancellation.
    signal: Cancellation,
    /// Currently installed input result sender.
    pending: RefCell<Option<oneshot::Sender<Result<String, LoginDialogError>>>>,
    /// Completion admission precedes external notification.
    completed: Cell<bool>,
    /// Caller notification invoked without any borrow.
    on_complete: Completion,
}
impl Settlement {
    /// Publishes abort, then rejection, then once-only completion.
    fn cancel(&self) {
        self.signal.abort();
        let pending = self.pending.borrow_mut().take();
        if let Some(sender) = pending {
            let _ = sender.send(Err(LoginDialogError::Cancelled));
        }
        if !self.completed.replace(true) {
            (self.on_complete)(false, Some("Login cancelled"));
        }
    }
}
/// Replaces every row of `input` in `content` by the plain text of the value submitted there,
/// unless either is gone.
fn freeze_input(content: &Weak<Container>, input: &Weak<Input>, value: &str) {
    let (Some(content), Some(input)) = (content.upgrade(), input.upgrade()) else {
        return;
    };
    let input: Rc<dyn Component> = input;
    let children = content.children();
    for child in children.borrow_mut().iter_mut() {
        if Rc::ptr_eq(child, &input) {
            *child = Rc::new(Text::new(format!("> {value}"), 0, 0, None));
        }
    }
}
/// Sign-in panel with inherited child-array editing.
pub struct LoginDialogComponent {
    /// Outer chrome and retained content slot.
    container: Container,
    /// Single input retained across visible and hidden states.
    input: Rc<Input>,
    /// Independent flag forwarding to the retained input.
    focus: FocusFlag,
    /// Input callbacks share only settlement state.
    settlement: Rc<Settlement>,
    /// Content retained independently of outer children.
    content: Rc<Container>,
    /// Presentation effects used by subsequent updates.
    options: LoginDialogOptions,
    /// Weak writer request supplied by the caller.
    request_render: Rc<dyn Fn()>,
}
impl LoginDialogComponent {
    /// Builds the title and borders using literal provider identity.
    ///
    /// # Errors
    /// Returns missing theme or foreground errors unchanged.
    pub fn new(
        request_render: Rc<dyn Fn()>,
        provider_id: &str,
        on_complete: Completion,
        mut options: LoginDialogOptions,
    ) -> Result<Self, ThemeError> {
        let title = options.title(provider_id)?;
        let container = Container::new();
        container.add_child(Rc::new(DynamicBorder::new(options.theme.clone(), None)));
        container.add_child(Rc::new(Text::new(title, 1, 0, None)));
        let content = Rc::new(Container::new());
        container.add_child(content.clone());
        container.add_child(Rc::new(DynamicBorder::new(options.theme.clone(), None)));
        let input = Rc::new(Input::new());
        let focus = FocusFlag::forwarding(input.clone());
        let settlement = Rc::new(Settlement {
            signal: Cancellation::new(),
            pending: RefCell::default(),
            completed: Cell::new(false),
            on_complete,
        });
        let cancel = settlement.clone();
        input.set_on_escape(Some(Rc::new(move || cancel.cancel())));
        Ok(Self {
            container,
            input,
            focus,
            settlement,
            content,
            options,
            request_render,
        })
    }
    /// Returns shared cancellation independently of input observers.
    #[must_use]
    pub fn signal(&self) -> &Cancellation {
        &self.settlement.signal
    }
    /// Adds the retained input and help, installing settlement before requesting render. A
    /// submitted value replaces the input row by its frozen text before the result is sent.
    fn input_result(&self, help: &str, accept_empty: bool) -> Result<LoginInputFuture, ThemeError> {
        let submit = self.settlement.clone();
        let content = Rc::downgrade(&self.content);
        let input = Rc::downgrade(&self.input);
        self.input.set_on_submit(Some(Rc::new(move |value| {
            if value.is_empty() && !accept_empty {
                return;
            }
            let pending = submit.pending.borrow_mut().take();
            if let Some(sender) = pending {
                freeze_input(&content, &input, value);
                let _ = sender.send(Ok(value.to_owned()));
            }
        })));
        self.content.add_child(self.input.clone());
        self.text(help, None)?;
        let (sender, receiver) = oneshot::channel();
        let old = self.settlement.pending.replace(Some(sender));
        drop(old);
        (self.request_render)();
        Ok(Box::pin(async move {
            receiver.await.unwrap_or(Err(LoginDialogError::Cancelled))
        }))
    }
    /// Captures the currently bound cancellation help.
    fn cancel_hint(&self, description: &str) -> Result<String, ThemeError> {
        maestro_app::key_hint("tui.select.cancel", description, &self.options.theme)
    }
    /// Appends manual entry with an empty input, keeping earlier rows.
    ///
    /// # Errors
    /// Returns theme errors unchanged.
    pub fn show_manual_input(&self, prompt: &str) -> Result<LoginInputFuture, ThemeError> {
        self.content.add_child(Rc::new(Spacer::new(1)));
        self.text(prompt, Some(ThemeColor::Dim))?;
        self.input.set_value(String::new());
        self.input_result(&format!("({})", self.cancel_hint("to cancel")?), false)
    }
    /// Appends a prompt and clears the retained input before installing its result.
    ///
    /// # Errors
    /// Returns theme errors unchanged.
    pub fn show_prompt(
        &self,
        message: &str,
        placeholder: Option<&str>,
    ) -> Result<LoginInputFuture, ThemeError> {
        self.content.add_child(Rc::new(Spacer::new(1)));
        self.text(message, Some(ThemeColor::Text))?;
        if let Some(placeholder) = placeholder.filter(|text| !text.is_empty()) {
            self.text(&format!("e.g., {placeholder}"), Some(ThemeColor::Dim))?;
        }
        let help = format!(
            "({} {})",
            self.cancel_hint("to cancel,")?,
            maestro_app::key_hint("tui.select.confirm", "to submit", &self.options.theme)?
        );
        self.input.set_value(String::new());
        self.input_result(&help, true)
    }
    /// Replaces visible rows with caller information without discarding pending input.
    ///
    /// # Errors
    /// Returns theme errors unchanged.
    pub fn show_info(&self, lines: &[String]) -> Result<(), ThemeError> {
        self.content.clear();
        self.content.add_child(Rc::new(Spacer::new(1)));
        for line in lines {
            self.text(line, None)?;
        }
        self.content.add_child(Rc::new(Spacer::new(1)));
        self.text(&format!("({})", self.cancel_hint("to close")?), None)?;
        (self.request_render)();
        Ok(())
    }
    /// Appends waiting text and cancellation help.
    ///
    /// # Errors
    /// Returns theme errors unchanged.
    pub fn show_waiting(&self, message: &str) -> Result<(), ThemeError> {
        self.content.add_child(Rc::new(Spacer::new(1)));
        self.text(message, Some(ThemeColor::Dim))?;
        self.text(&format!("({})", self.cancel_hint("to cancel")?), None)?;
        (self.request_render)();
        Ok(())
    }
    /// Appends only a progress message, retaining all earlier content.
    ///
    /// # Errors
    /// Returns theme errors unchanged.
    pub fn show_progress(&self, message: &str) -> Result<(), ThemeError> {
        self.text(message, Some(ThemeColor::Dim))?;
        (self.request_render)();
        Ok(())
    }
    /// Adds a captured text row with optional foreground.
    fn text(&self, text: &str, color: Option<ThemeColor>) -> Result<(), ThemeError> {
        let text = color.map_or_else(
            || Ok(text.to_owned()),
            |color| self.options.theme.get()?.fg(&color, text),
        )?;
        self.content.add_child(Rc::new(Text::new(text, 1, 0, None)));
        Ok(())
    }
    /// Replaces visible content with an authorization link and launches the browser.
    ///
    /// # Errors
    /// Returns theme errors unchanged; browser startup and exit failures are silent.
    pub fn show_auth(&self, url: &str, instructions: Option<&str>) -> Result<(), ThemeError> {
        self.content.clear();
        self.content.add_child(Rc::new(Spacer::new(1)));
        let linked_url = format!("\x1b]8;;{url}\x07{url}\x1b]8;;\x07");
        self.text(&linked_url, Some(ThemeColor::Accent))?;
        let hint = match self.options.platform {
            LoginDialogPlatform::MacOs => "Cmd+click to open",
            _ => "Ctrl+click to open",
        };
        let hyperlink = format!("\x1b]8;;{url}\x07{hint}\x1b]8;;\x07");
        self.text(&hyperlink, Some(ThemeColor::Dim))?;
        if let Some(instructions) = instructions.filter(|text| !text.is_empty()) {
            self.content.add_child(Rc::new(Spacer::new(1)));
            self.text(instructions, Some(ThemeColor::Warning))?;
        }
        if let Some(launch) = &self.options.launch_browser {
            launch(url);
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let program = match self.options.platform {
                    LoginDialogPlatform::MacOs => "open",
                    LoginDialogPlatform::Windows => "explorer.exe",
                    LoginDialogPlatform::Other => "xdg-open",
                };
                let mut command = tokio::process::Command::new(program);
                command.arg(url);
                let _ = maestro_app::spawn(command, None);
            }
        }
        (self.request_render)();
        Ok(())
    }
}
component_children!(LoginDialogComponent, Container, container);
impl Component for LoginDialogComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

impl Focusable for LoginDialogComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for LoginDialogComponent {
    fn handle_input(&self, data: &str) {
        if maestro_tui::get_keybindings().matches(data, "tui.select.cancel") {
            self.settlement.cancel();
        } else {
            self.input.handle_input(data);
        }
    }
}
