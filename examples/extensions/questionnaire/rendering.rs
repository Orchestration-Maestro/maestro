//! Questionnaire frames and compact tool-call/result summaries.
use super::{
    QuestionnaireResult,
    interface::{Questionnaire, State},
};
use maestro_extensions_wasm::{
    Component, ExtensionAPI, ExtensionResult, Presence, Text, Theme, ToolRenderResultLike,
    UserBlock, truncate_to_width, utils::TruncateOptions,
};
use serde_json::Value;

/// Styles and clips each nonempty authored row through the host's cell-width owner.
pub(super) fn view(view: &Questionnaire, width: u32) -> ExtensionResult<Vec<String>> {
    let state = view.state.borrow();
    let theme = &view.theme;
    let border = theme.fg("accent", &"─".repeat(width as usize))?;
    let mut lines = vec![border.clone()];
    if view.questions.len() > 1 {
        lines.push(tabs(view, &state)?);
        lines.push(String::new());
    }
    if let Some(question) = view.questions.get(state.tab) {
        lines.push(theme.fg("text", &format!(" {}", question.prompt))?);
        lines.push(String::new());
        options(view, &state, &mut lines)?;
        if state.input.is_some() {
            lines.extend([String::new(), theme.fg("muted", " Your answer:")?]);
            lines.extend(
                view.editor
                    .render(width.saturating_sub(2))?
                    .into_iter()
                    .map(|line| format!(" {line}")),
            );
            lines.extend([
                String::new(),
                theme.fg("dim", " Enter to submit • Esc to cancel")?,
            ]);
        }
    } else {
        summary(view, &state, &mut lines)?;
    }
    lines.push(String::new());
    if state.input.is_none() {
        lines.push(theme.fg(
            "dim",
            if view.questions.len() > 1 {
                " Tab/←→ navigate • ↑↓ select • Enter confirm • Esc cancel"
            } else {
                " ↑↓ navigate • Enter select • Esc cancel"
            },
        )?);
    }
    lines.push(border);
    lines
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                Ok(line)
            } else {
                truncate_to_width(&view.api, &line, width as usize, TruncateOptions::default())
            }
        })
        .collect()
}
/// Tab labels indicate both selection and whether an answer exists.
fn tabs(view: &Questionnaire, state: &State) -> ExtensionResult<String> {
    let mut text = " ← ".to_owned();
    for (index, question) in view.questions.iter().enumerate() {
        let answered = state.answers.iter().any(|a| a.id == question.id);
        let label = format!(" {} {} ", if answered { "■" } else { "□" }, question.label);
        let styled = if index == state.tab {
            view.theme
                .bg("selectedBg", &view.theme.fg("text", &label)?)?
        } else {
            view.theme
                .fg(if answered { "success" } else { "muted" }, &label)?
        };
        text.push_str(&styled);
        text.push(' ');
    }
    let submit = if state.tab == view.questions.len() {
        view.theme
            .bg("selectedBg", &view.theme.fg("text", " ✓ Submit ")?)?
    } else {
        view.theme.fg(
            if view.all_answered(state) {
                "success"
            } else {
                "dim"
            },
            " ✓ Submit ",
        )?
    };
    text.push_str(&submit);
    text.push_str(" →");
    Ok(text)
}
/// Draws supplied options followed by the optional custom-answer choice.
fn options(view: &Questionnaire, state: &State, lines: &mut Vec<String>) -> ExtensionResult<()> {
    let question = &view.questions[state.tab];
    let labels = question
        .options
        .iter()
        .map(|o| (o.label.as_str(), o.description.as_deref()))
        .chain(question.allow_other.then_some(("Type something.", None)));
    for (index, (label, description)) in labels.enumerate() {
        let selected = index == state.option;
        let prefix = if selected {
            view.theme.fg("accent", "> ")?
        } else {
            "  ".into()
        };
        let editing = index == question.options.len() && state.input.is_some();
        let role = if selected || editing {
            "accent"
        } else {
            "text"
        };
        let suffix = if editing { " ✎" } else { "" };
        lines.push(
            prefix
                + &view
                    .theme
                    .fg(role, &format!("{}. {label}{suffix}", index + 1))?,
        );
        if let Some(description) = description.filter(|s| !s.is_empty()) {
            lines.push(format!("     {}", view.theme.fg("muted", description)?));
        }
    }
    Ok(())
}
/// Shows answers and the remaining questions before allowing submission.
fn summary(view: &Questionnaire, state: &State, lines: &mut Vec<String>) -> ExtensionResult<()> {
    lines.extend([
        view.theme
            .fg("accent", &view.theme.bold(" Ready to submit")?)?,
        String::new(),
    ]);
    for question in &view.questions {
        if let Some(answer) = state.answers.iter().find(|a| a.id == question.id) {
            let prefix = if answer.was_custom { "(wrote) " } else { "" };
            lines.push(
                view.theme.fg("muted", &format!(" {}: ", question.label))?
                    + &view
                        .theme
                        .fg("text", &format!("{prefix}{}", answer.label))?,
            );
        }
    }
    lines.push(String::new());
    lines.push(if view.all_answered(state) {
        view.theme.fg("success", " Press Enter to submit")?
    } else {
        let missing = view
            .questions
            .iter()
            .filter(|q| !state.answers.iter().any(|a| a.id == q.id))
            .map(|q| q.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        view.theme
            .fg("warning", &format!(" Unanswered: {missing}"))?
    });
    Ok(())
}
/// Draws the question count and shortened supplied labels while arguments arrive.
pub(super) fn call(
    api: &ExtensionAPI,
    args: Option<&Value>,
    theme: &Theme,
) -> ExtensionResult<Component> {
    let questions = args
        .and_then(|a| a["questions"].as_array())
        .cloned()
        .unwrap_or_default();
    let count = questions.len();
    let labels = questions
        .iter()
        .map(|q| {
            q["label"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| q["id"].as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut text = theme.fg("toolTitle", &theme.bold("questionnaire ")?)?
        + &theme.fg(
            "muted",
            &format!("{count} question{}", if count == 1 { "" } else { "s" }),
        )?;
    if !labels.is_empty() {
        text.push_str(&theme.fg(
            "dim",
            &format!(
                " ({})",
                truncate_to_width(api, &labels, 40, TruncateOptions::default())?
            ),
        )?);
    }
    Ok(Component::from(&Text::new(&text, 0, 0, None)?))
}
/// Draws cancellation or each selected/custom answer; absent details fall back to text.
pub(super) fn result(result: &ToolRenderResultLike, theme: &Theme) -> ExtensionResult<Component> {
    let text = if let Presence::Present(details) = &result.result.details {
        let details: QuestionnaireResult =
            serde_json::from_str(details).map_err(|e| e.to_string())?;
        if details.cancelled {
            theme.fg("warning", "Cancelled")?
        } else {
            details
                .answers
                .iter()
                .map(|answer| answer_line(answer, theme))
                .collect::<ExtensionResult<Vec<_>>>()?
                .join("\n")
        }
    } else {
        match result.result.content.first() {
            Some(UserBlock::Text(text)) => text.text.clone(),
            _ => String::new(),
        }
    };
    Ok(Component::from(&Text::new(&text, 0, 0, None)?))
}

/// Formats one answer for the compact result renderer.
fn answer_line(answer: &super::Answer, theme: &Theme) -> ExtensionResult<String> {
    let prefix = theme.fg("success", "✓ ")? + &theme.fg("accent", &answer.id)? + ": ";
    let display = if answer.was_custom {
        theme.fg("muted", "(wrote) ")? + &answer.label
    } else {
        answer.index.map_or_else(
            || answer.label.clone(),
            |index| format!("{index}. {}", answer.label),
        )
    };
    Ok(prefix + &display)
}
