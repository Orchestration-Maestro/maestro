//! Retained option navigation with the host's multiline editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Params, Selection};
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactoryOutcome, Editor,
    EditorOptions, EditorTheme, ExtensionAPI, ExtensionComponent, ExtensionContext,
    ExtensionResult, SelectListTheme, Style, TUI, Theme, matches_key,
    utils::{TruncateOptions, is_whitespace_char, truncate_to_width},
};
use std::{cell::RefCell, rc::Rc};

/// Mutable navigation, kept separate from reentrant host effects.
#[derive(Default)]
struct State {
    /// Selected supplied option or the final custom-input row.
    option: usize,
    /// Whether input goes to the inline editor.
    editing: bool,
}
/// One custom view and its retained host effects.
struct QuestionView {
    /// Host-backed helpers.
    api: ExtensionAPI,
    /// Caller-authored question and choices.
    params: Params,
    /// Current navigation and editor mode.
    state: RefCell<State>,
    /// Host-owned editing behavior.
    editor: Editor,
    /// Theme supplied to this view.
    theme: Theme,
    /// Frame writer to request refreshes.
    tui: TUI,
    /// Completes the view with an answer or cancellation.
    done: CustomDone<Option<Selection>>,
}
impl QuestionView {
    /// Uses the shared whitespace definition while preserving the interior text.
    fn submit(&self, text: &str) -> ExtensionResult<()> {
        let mut start = 0;
        for (index, ch) in text.char_indices() {
            if !is_whitespace_char(&self.api, &ch.to_string())? {
                break;
            }
            start = index + ch.len_utf8();
        }
        let mut end = text.len();
        for (index, ch) in text[start..].char_indices().rev() {
            if !is_whitespace_char(&self.api, &ch.to_string())? {
                break;
            }
            end = start + index;
        }
        let answer = &text[start..end.max(start)];
        if answer.is_empty() {
            self.state.borrow_mut().editing = false;
            self.editor.set_text("")?;
            self.tui.request_render(false)
        } else {
            (self.done)(Some(Selection {
                answer: answer.into(),
                was_custom: true,
                index: None,
            }))
        }
    }
    /// Routes editing keys separately so Escape returns to the options first.
    fn input(&self, data: &str) -> ExtensionResult<()> {
        if self.state.borrow().editing {
            if matches_key(&self.api, data, "escape")? {
                self.state.borrow_mut().editing = false;
                self.editor.set_text("")?;
            } else {
                self.editor.handle_input(data)?;
            }
            return self.tui.request_render(false);
        }
        if matches_key(&self.api, data, "up")? {
            let mut state = self.state.borrow_mut();
            state.option = state.option.saturating_sub(1);
        } else if matches_key(&self.api, data, "down")? {
            let mut state = self.state.borrow_mut();
            state.option = (state.option + 1).min(self.params.options.len());
        } else if matches_key(&self.api, data, "enter")? {
            let option = self.state.borrow().option;
            if let Some(selected) = self.params.options.get(option) {
                return (self.done)(Some(Selection {
                    answer: selected.label.clone(),
                    was_custom: false,
                    index: Some(option + 1),
                }));
            }
            self.state.borrow_mut().editing = true;
        } else if matches_key(&self.api, data, "escape")? {
            return (self.done)(None);
        } else {
            return Ok(());
        }
        self.tui.request_render(false)
    }
    /// Paints one numbered option, including the inline editor's active marker.
    fn option(&self, index: usize, label: &str) -> ExtensionResult<String> {
        let state = self.state.borrow();
        let selected = index == state.option;
        let prefix = if selected {
            self.theme.fg("accent", "> ")?
        } else {
            "  ".into()
        };
        let marker = if index == self.params.options.len() && state.editing {
            " ✎"
        } else {
            ""
        };
        let text = format!("{}. {label}{marker}", index + 1);
        Ok(prefix
            + &self
                .theme
                .fg(if selected { "accent" } else { "text" }, &text)?)
    }
    /// Renders afresh at the supplied width, including on resize without invalidation.
    fn rows(&self, columns: u32) -> ExtensionResult<Vec<String>> {
        let width = usize::try_from(columns).map_err(|e| e.to_string())?;
        let border = self.theme.fg("accent", &"─".repeat(width))?;
        let mut rows = vec![
            border.clone(),
            self.theme
                .fg("text", &format!(" {}", self.params.question))?,
            String::new(),
        ];
        for (index, option) in self.params.options.iter().enumerate() {
            rows.push(self.option(index, &option.label)?);
            if let Some(description) = option.description.as_ref().filter(|d| !d.is_empty()) {
                rows.push(format!("     {}", self.theme.fg("muted", description)?));
            }
        }
        rows.push(self.option(self.params.options.len(), "Type something.")?);
        let editing = self.state.borrow().editing;
        if editing {
            rows.extend([String::new(), self.theme.fg("muted", " Your answer:")?]);
            rows.extend(
                self.editor
                    .render(columns.saturating_sub(2))?
                    .into_iter()
                    .map(|line| format!(" {line}")),
            );
        }
        rows.push(String::new());
        let hint = if editing {
            " Enter to submit • Esc to go back"
        } else {
            " ↑↓ navigate • Enter to select • Esc to cancel"
        };
        rows.extend([self.theme.fg("dim", hint)?, border]);
        rows.into_iter()
            .map(|row| truncate_to_width(&self.api, &row, width, TruncateOptions::default()))
            .collect()
    }
}
/// Shares the view with its weak editor submission handler.
struct View(Rc<QuestionView>);
impl ComponentView for View {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.rows(width)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.editor.invalidate()
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.input(data)
    }
}
/// Gives the inline editor the custom view's supplied semantic styles.
fn editor_theme(theme: &Theme) -> EditorTheme {
    let style = |role: &'static str| {
        let theme = theme.clone();
        Style::new(move |text| theme.fg(role, text))
    };
    EditorTheme {
        border_color: style("accent"),
        select_list: SelectListTheme {
            selected_prefix: style("accent"),
            selected_text: style("accent"),
            description: style("muted"),
            scroll_info: style("dim"),
            no_match: style("warning"),
        },
    }
}
/// Retains one editor until the host closes its custom view.
pub(super) async fn ask(
    api: ExtensionAPI,
    params: Params,
    ctx: &ExtensionContext,
) -> ExtensionResult<Option<Selection>> {
    Ok(ctx
        .ui()?
        .custom(
            Rc::new(move |tui, theme, _, done, _| {
                let editor = Editor::new(&tui, &editor_theme(&theme), EditorOptions::default())?;
                let view = Rc::new(QuestionView {
                    api: api.clone(),
                    params: params.clone(),
                    state: RefCell::new(State::default()),
                    editor,
                    theme,
                    tui,
                    done,
                });
                let weak = Rc::downgrade(&view);
                view.editor.set_on_submit(Some(Rc::new(move |text, _| {
                    weak.upgrade().map_or(Ok(()), |view| view.submit(text))
                })))?;
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(View(view)),
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }))
            }),
            None,
        )
        .await?
        .flatten())
}
