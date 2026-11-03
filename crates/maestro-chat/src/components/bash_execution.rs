//! Retained direct shell command rows.
use super::dynamic_border::DynamicBorder;
use maestro_app::{
    DiagnosticErrorInfo, TruncationOptions, TruncationResult, key_hint, key_text, normalize_to_lf,
    num, plain_key_hint, strip_ansi, truncate_tail, truncate_to_visual_lines,
};
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{Component, Container, Loader, Spacer, TUI, Text};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Direct shell command display with editable retained children.
pub struct BashExecutionComponent {
    /// Existing outer child owner.
    container: Container,
    /// Immutable authored command.
    command: String,
    /// Dynamic content between the borders.
    content: Rc<Container>,
    /// Shared animation owner.
    loader: Rc<Loader>,
    /// Live styling slot.
    theme: LiveTheme,
    /// Constructor-selected command role.
    role: ThemeColor,
    /// Full cleaned output.
    output: RefCell<String>,
    /// Requested output expansion.
    expanded: Cell<bool>,
    /// Latest final status and artifact operands.
    completion: RefCell<Option<Rc<Completion>>>,
}
impl BashExecutionComponent {
    /// Retains the command and starts the existing loader.
    #[must_use]
    pub fn new(command: String, ui: TUI, theme: LiveTheme, exclude_from_context: bool) -> Self {
        let role = if exclude_from_context {
            ThemeColor::Dim
        } else {
            ThemeColor::BashMode
        };
        let border_theme = theme.clone();
        let border_role = role.clone();
        let color: Rc<dyn Fn(&str) -> String> =
            Rc::new(move |text| paint(border_theme.get().ok().as_deref(), &border_role, text));
        let message_theme = theme.clone();
        let loader = Rc::new(Loader::new(
            ui,
            color.clone(),
            Rc::new(move |text| {
                paint(
                    message_theme.get().ok().as_deref(),
                    &ThemeColor::Muted,
                    text,
                )
            }),
            Some(format!(
                "Running... ({} to cancel)",
                key_text("tui.select.cancel")
            )),
            None,
        ));
        let container = Container::new();
        let content = Rc::new(Container::new());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(
            theme.clone(),
            Some(color.clone()),
        )));
        container.add_child(content.clone());
        container.add_child(Rc::new(DynamicBorder::new(theme.clone(), Some(color))));
        let display = Self {
            container,
            command,
            content,
            loader,
            theme,
            role,
            output: RefCell::new(String::new()),
            expanded: Cell::new(false),
            completion: RefCell::new(None),
        };
        display.content.add_child(display.header());
        display.content.add_child(display.loader.clone());
        display
    }
    /// Captures the selected role and authored command in the shared text widget.
    fn header(&self) -> Rc<Text> {
        Rc::new(Text::new(
            self.paint(
                &self.role,
                &maestro_theme::Theme::bold(&format!("$ {}", self.command), true),
            ),
            1,
            0,
            None,
        ))
    }
    /// Cleans one chunk and refreshes the retained rows.
    ///
    /// # Errors
    /// Propagates the ANSI stripper diagnostic before changing output.
    pub fn append_output(&self, chunk: &str) -> Result<(), DiagnosticErrorInfo> {
        let clean = normalize_to_lf(&strip_ansi(chunk)?);
        self.output.borrow_mut().push_str(&clean);
        self.rebuild();
        Ok(())
    }
    /// Returns the full cleaned output, not the display tail.
    #[must_use]
    pub fn get_output(&self) -> String {
        self.output.borrow().clone()
    }
    /// Rebuilds the context-limited preview without restarting animation.
    fn rebuild(&self) {
        let context = truncate_tail(&self.output.borrow(), TruncationOptions::default());
        let lines: Vec<_> = if context.content.is_empty() {
            vec![]
        } else {
            context.content.split('\n').collect()
        };
        let preview = &lines[lines.len().saturating_sub(20)..];
        self.content.clear();
        self.content.add_child(self.header());
        if !lines.is_empty() {
            let selected = if self.expanded.get() {
                lines.as_slice()
            } else {
                preview
            };
            let styled = selected
                .iter()
                .map(|line| self.paint(&ThemeColor::Muted, line))
                .collect::<Vec<_>>()
                .join("\n");
            let text = format!("\n{styled}");
            if self.expanded.get() {
                self.content.add_child(Rc::new(Text::new(text, 1, 0, None)));
            } else {
                self.content.add_child(Rc::new(Preview {
                    text,
                    cache: RefCell::new(None),
                }));
            }
        }
        if let Some(notices) = self.notices(lines.len() - preview.len(), context.truncated) {
            if !notices.is_empty() {
                self.content
                    .add_child(Rc::new(Text::new(format!("\n{notices}"), 1, 0, None)));
            }
        } else {
            self.content.add_child(self.loader.clone());
        }
    }
    /// Rebuilds output even when expansion is unchanged.
    pub fn set_expanded(&self, expanded: bool) {
        self.expanded.set(expanded);
        self.rebuild();
    }
    /// Replaces completion data and stops the retained loader.
    pub fn set_complete(
        &self,
        exit_code: Option<f64>,
        cancelled: bool,
        truncation_result: Option<TruncationResult>,
        full_output_path: Option<String>,
    ) {
        let kind = if cancelled {
            CompletionKind::Cancelled
        } else if let Some(code) = exit_code.filter(|code| *code != 0.0) {
            CompletionKind::Error(code)
        } else {
            CompletionKind::Success
        };
        let old = self.completion.replace(Some(Rc::new(Completion {
            kind,
            truncated: truncation_result.is_some_and(|result| result.truncated),
            path: full_output_path,
        })));
        drop(old);
        self.loader.stop();
        self.rebuild();
    }
    /// Samples expansion keys and the currently available styling.
    fn hint(&self, description: &str) -> String {
        key_hint("app.tools.expand", description, &self.theme)
            .unwrap_or_else(|_| plain_key_hint("app.tools.expand", description))
    }
    /// Captures final notices in their displayed order without a state borrow.
    fn notices(&self, hidden: usize, context_truncated: bool) -> Option<String> {
        let completion = self.completion.borrow().as_ref().map(Rc::clone)?;
        let mut parts = Vec::new();
        if hidden > 0 {
            let (open, hint) = if self.expanded.get() {
                ("(".to_owned(), self.hint("to collapse"))
            } else {
                (format!("... {hidden} more lines ("), self.hint("to expand"))
            };
            parts.push(format!(
                "{}{hint}{}",
                self.paint(&ThemeColor::Muted, &open),
                self.paint(&ThemeColor::Muted, ")")
            ));
        }
        match completion.kind {
            CompletionKind::Cancelled => {
                parts.push(self.paint(&ThemeColor::Warning, "(cancelled)"));
            }
            CompletionKind::Error(code) => {
                parts.push(self.paint(&ThemeColor::Error, &format!("(exit {})", num(code))));
            }
            CompletionKind::Success => {}
        }
        if let Some(path) = completion
            .path
            .as_deref()
            .filter(|path| !path.is_empty() && (completion.truncated || context_truncated))
        {
            parts.push(self.paint(
                &ThemeColor::Warning,
                &format!("Output truncated. Full output: {path}"),
            ));
        }
        Some(parts.join("\n"))
    }
    /// Borrows the unchanged command.
    #[must_use]
    pub fn get_command(&self) -> &str {
        &self.command
    }
    /// Omits unavailable theme colours while preserving existing decorations.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.theme.get().ok().as_deref(), key, text)
    }
}
component_children!(BashExecutionComponent, Container, container);
impl Component for BashExecutionComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.container.try_render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
        self.rebuild();
    }
}

/// Styled logical preview with render-width-specific owned rows.
struct Preview {
    /// Captured styled input.
    text: String,
    /// Last width and its selected wrapped rows.
    cache: RefCell<Option<(usize, Vec<String>)>>,
}
impl Component for Preview {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some((cached_width, rows)) = self.cache.borrow().as_ref()
            && *cached_width == width
        {
            return rows.clone();
        }
        let rows = truncate_to_visual_lines(&self.text, 20, width, Some(1)).visual_lines;
        self.cache.replace(Some((width, rows.clone())));
        rows
    }
    fn invalidate(&self) {
        self.cache.take();
    }
}

/// Final display kind with the original numeric exit capability.
#[derive(Clone, Copy)]
enum CompletionKind {
    /// Successful completion contributes no notice.
    Success,
    /// Cancellation takes precedence over exit errors.
    Cancelled,
    /// Nonzero caller-supplied exit code.
    Error(f64),
}
/// Only final operands consumed by this display.
struct Completion {
    /// Selected final kind.
    kind: CompletionKind,
    /// Externally supplied truncation flag.
    truncated: bool,
    /// Authored artifact spelling.
    path: Option<String>,
}
