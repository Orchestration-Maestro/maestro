//! Retained questionnaire navigation and host-backed inline editor.
use super::{Answer, Question, QuestionnaireResult};
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactoryOutcome, Editor,
    EditorOptions, EditorTheme, ExtensionAPI, ExtensionComponent, ExtensionContext,
    ExtensionResult, SelectListTheme, Style, TUI, Theme, matches_key,
};
use std::{cell::RefCell, rc::Rc};

/// Mutable selection and answers, independent of the host editor.
#[derive(Default)]
pub(super) struct State {
    /// Active question or final submit tab.
    pub(super) tab: usize,
    /// Selected option, reset on every tab change.
    pub(super) option: usize,
    /// Question receiving editor input, when active.
    pub(super) input: Option<String>,
    /// Answers kept in insertion order even when replaced.
    pub(super) answers: Vec<Answer>,
}
/// The view holds host effects outside its mutable state borrow.
pub(super) struct Questionnaire {
    /// Host helper access.
    pub(super) api: ExtensionAPI,
    /// Normalized question inventory.
    pub(super) questions: Vec<Question>,
    /// Navigation and answer state.
    pub(super) state: RefCell<State>,
    /// Host multiline editor.
    pub(super) editor: Editor,
    /// Factory-supplied theme.
    pub(super) theme: Theme,
    /// Frame writer to request refreshes.
    tui: TUI,
    /// Completes the custom view once.
    done: CustomDone<QuestionnaireResult>,
}
impl Questionnaire {
    /// Whether every question has an answer, including repeated identifiers.
    pub(super) fn all_answered(&self, state: &State) -> bool {
        self.questions
            .iter()
            .all(|q| state.answers.iter().any(|a| a.id == q.id))
    }
    /// Finishes with partial answers retained on cancellation.
    fn submit(&self, cancelled: bool) -> ExtensionResult<()> {
        (self.done)(QuestionnaireResult {
            questions: self.questions.clone(),
            answers: self.state.borrow().answers.clone(),
            cancelled,
        })
    }
    /// Replaces an answer without changing its original position.
    fn save(&self, answer: Answer) {
        let mut state = self.state.borrow_mut();
        if let Some(old) = state.answers.iter_mut().find(|a| a.id == answer.id) {
            *old = answer;
        } else {
            state.answers.push(answer);
        }
    }
    /// Single questions finish immediately; multiple questions advance toward Submit.
    fn advance(&self) -> ExtensionResult<()> {
        if self.questions.len() == 1 {
            return self.submit(false);
        }
        {
            let mut state = self.state.borrow_mut();
            state.tab += 1;
            state.option = 0;
        }
        self.tui.request_render(false)
    }
    /// Trims custom text with the host's shared whitespace set.
    fn custom_answer(&self, text: &str) -> ExtensionResult<()> {
        let Some(id) = self.state.borrow_mut().input.take() else {
            return Ok(());
        };
        let mut start = 0;
        let mut end = text.len();
        for (index, ch) in text.char_indices() {
            if !maestro_extensions_wasm::utils::is_whitespace_char(&self.api, &ch.to_string())? {
                break;
            }
            start = index + ch.len_utf8();
        }
        for (index, ch) in text[start..].char_indices().rev() {
            if !maestro_extensions_wasm::utils::is_whitespace_char(&self.api, &ch.to_string())? {
                break;
            }
            end = start + index;
        }
        let trimmed = &text[start..end.max(start)];
        let value = if trimmed.is_empty() {
            "(no response)"
        } else {
            trimmed
        };
        self.save(Answer {
            id,
            value: value.into(),
            label: value.into(),
            was_custom: true,
            index: None,
        });
        self.editor.set_text("")?;
        self.advance()
    }
    /// Changes tabs with wraparound, resetting the option selection.
    fn navigate(&self, data: &str) -> ExtensionResult<bool> {
        if self.questions.len() == 1 {
            return Ok(false);
        }
        let forward =
            matches_key(&self.api, data, "tab")? || matches_key(&self.api, data, "right")?;
        let backward = !forward
            && (matches_key(&self.api, data, "shift+tab")?
                || matches_key(&self.api, data, "left")?);
        if !forward && !backward {
            return Ok(false);
        }
        let mut state = self.state.borrow_mut();
        let count = self.questions.len() + 1;
        state.tab = (state.tab + if forward { 1 } else { count - 1 }) % count;
        state.option = 0;
        drop(state);
        self.tui.request_render(false)?;
        Ok(true)
    }
    /// Selects a supplied option or opens the inline editor.
    fn select(&self) -> ExtensionResult<()> {
        let (tab, option) = {
            let state = self.state.borrow();
            (state.tab, state.option)
        };
        let question = &self.questions[tab];
        if let Some(answer) = question.options.get(option) {
            self.save(Answer {
                id: question.id.clone(),
                value: answer.value.clone(),
                label: answer.label.clone(),
                was_custom: false,
                index: Some(option + 1),
            });
            self.advance()
        } else if question.allow_other {
            self.state.borrow_mut().input = Some(question.id.clone());
            self.editor.set_text("")?;
            self.tui.request_render(false)
        } else {
            Ok(())
        }
    }
    /// Moves within the current question without wrapping past either end.
    fn move_option(&self, up: bool) -> ExtensionResult<()> {
        let mut state = self.state.borrow_mut();
        let q = &self.questions[state.tab];
        let last = (q.options.len() + usize::from(q.allow_other)).saturating_sub(1);
        state.option = if up {
            state.option.saturating_sub(1)
        } else {
            (state.option + 1).min(last)
        };
        drop(state);
        self.tui.request_render(false)
    }
    /// Processes non-editor input without holding state across host calls.
    fn input(&self, data: &str) -> ExtensionResult<()> {
        if self.navigate(data)? {
            return Ok(());
        }
        let tab = self.state.borrow().tab;
        if tab == self.questions.len() {
            if matches_key(&self.api, data, "enter")? && self.all_answered(&self.state.borrow()) {
                return self.submit(false);
            }
        } else {
            let up = matches_key(&self.api, data, "up")?;
            let down = !up && matches_key(&self.api, data, "down")?;
            if up || down {
                return self.move_option(up);
            }
            if matches_key(&self.api, data, "enter")? {
                return self.select();
            }
        }
        if matches_key(&self.api, data, "escape")? {
            self.submit(true)?;
        }
        Ok(())
    }
}
/// Owned component wrapper around shared editor callback state.
struct View(Rc<Questionnaire>);
impl std::ops::Deref for View {
    type Target = Questionnaire;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ComponentView for View {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        super::rendering::view(self, width)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.editor.invalidate()
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.state.borrow().input.is_none() {
            return self.input(data);
        }
        if matches_key(&self.api, data, "escape")? {
            self.state.borrow_mut().input = None;
            self.editor.set_text("")?;
        } else {
            self.editor.handle_input(data)?;
        }
        self.tui.request_render(false)
    }
}
/// Styles the inline editor with the custom view's supplied theme.
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
/// Opens a custom component and retains its editor until the host closes it.
pub(super) async fn ask(
    api: ExtensionAPI,
    questions: Vec<Question>,
    ctx: &ExtensionContext,
) -> ExtensionResult<QuestionnaireResult> {
    ctx.ui()?
        .custom(
            Rc::new(move |tui, theme, _, done, _| {
                let editor = Editor::new(&tui, &editor_theme(&theme), EditorOptions::default())?;
                let view = Rc::new(Questionnaire {
                    api: api.clone(),
                    questions: questions.clone(),
                    state: RefCell::new(State::default()),
                    editor,
                    theme,
                    tui,
                    done,
                });
                let weak = Rc::downgrade(&view);
                view.editor.set_on_submit(Some(Rc::new(move |text, _| {
                    weak.upgrade()
                        .map_or(Ok(()), |view| view.custom_answer(text))
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
        .ok_or_else(|| "Questionnaire closed without a result".into())
}
