//! Strict recorded message constructor and operation replay.
mod input;
mod palette;
#[allow(dead_code, reason = "Only publication helpers are shared.")]
#[path = "../../../maestro-theme/tests/published/mod.rs"]
mod published;
mod widget;
use input::{Action, Case};
use maestro_tui::{Component, KeybindingsManager, get_keybindings, set_keybindings};
use std::{collections::HashSet, rc::Rc, sync::Mutex};
use widget::Widget;

/// Serializes replacements of the active key registry.
static REGISTRY: Mutex<()> = Mutex::new(());
/// Restores the registry even when an assertion fails.
struct Restore(KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
/// Default expand action with caller-supplied ordered keys.
fn bind(keys: Vec<String>) {
    use maestro_tui::{KeybindingDefinition, KeybindingKeys};
    set_keybindings(KeybindingsManager::new(
        vec![(
            "app.tools.expand".into(),
            KeybindingDefinition {
                default_keys: KeybindingKeys::Multiple(keys),
                description: None,
            },
        )],
        vec![],
    ));
}
/// Replays one complete recorded invocation, preserving frame order.
fn replay(case: &Case) {
    let input = &case.input;
    let (state, live) =
        published::published(&palette::palette(input.palette.as_deref().unwrap_or("a")));
    bind(vec!["ctrl+o".into()]);
    let enabled = input.styles_enabled.unwrap_or(true);
    let mut markdown = maestro_theme::get_markdown_theme(
        live.clone(),
        Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        enabled,
    );
    if input.custom_markdown.unwrap_or(false) {
        markdown.heading = Box::new(|text| format!("CUSTOM[{text}]"));
    }
    let (widget, mut retained) = Widget::new(input, live, Rc::new(markdown));
    let mut frames = vec![widget.render(input.width)];
    let mut alias = None;
    for operation in &input.ops {
        match &operation.action {
            Action::Update { message } => {
                let message = widget::message(Some(message));
                widget.assistant().update_content(message.clone());
                retained = Some(message);
            }
            Action::Mutate { message } => {
                widget::edit(&mut retained.as_ref().unwrap().write().unwrap(), message);
            }
            Action::Hide { value } => widget.assistant().set_hide_thinking_block(*value),
            Action::Label { value } => widget.assistant().set_hidden_thinking_label(value.clone()),
            Action::Expand { value } => widget.expand(*value),
            Action::Invalidate => widget.invalidate(),
            Action::Theme { value } => {
                state.set_theme_instance(palette::palette(value)).unwrap();
            }
            Action::Keys { value } => bind(value.clone()),
            Action::CaptureChildren => alias = Some(widget.children()),
            Action::Clear => widget.clear(),
            Action::AppendAlias { text } => alias
                .as_ref()
                .unwrap()
                .borrow_mut()
                .push(Rc::new(maestro_tui::Text::new(text.clone(), 0, 0, None))),
            Action::Append { text } => widget.append(text.clone()),
        }
        if operation.render != Some(false) {
            frames.push(widget.render(input.width));
        }
    }
    assert_eq!(frames, case.expected, "scenario {}", case.name);
}
/// Replays only this test's cases and rejects duplicate queries or case identities.
pub fn group(name: &str) {
    let _serial = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    let mut cases: Vec<Case> = Vec::new();
    let mut queries = HashSet::new();
    for source in [
        include_str!("../fixtures/message_rendering.json"),
        include_str!("../fixtures/summary_locales.json"),
    ] {
        let raw: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
        for row in raw {
            assert!(queries.insert(serde_json::to_string(&row["input"]).unwrap()));
            cases.push(serde_json::from_value(row).unwrap());
        }
    }
    let identities: HashSet<_> = cases.iter().map(|case| (&case.test, &case.name)).collect();
    assert_eq!(identities.len(), cases.len());
    assert_eq!(cases.len(), 147);
    let groups: HashSet<_> = cases.iter().map(|case| case.test.as_str()).collect();
    assert_eq!(groups, super::FIXTURE_GROUPS.iter().copied().collect());
    let selected: Vec<_> = cases.iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    for case in selected {
        replay(case);
    }
}

/// Weak handles prove replacement and final-drop release without a clock.
pub fn releases_handles() {
    let (_, live) = published::published(&palette::palette("a"));
    let markdown = Rc::new(maestro_theme::get_markdown_theme(
        live.clone(),
        Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        true,
    ));
    let original = widget::message(None);
    let old = std::sync::Arc::downgrade(&original);
    let component = maestro_chat::AssistantMessageComponent::new(
        Some(original),
        maestro_chat::AssistantMessageOptions::default(),
        live,
        markdown,
        maestro_tui::TerminalImage::new(|_| None, || 1),
    );
    assert!(old.upgrade().is_some());
    let replacement = widget::message(None);
    let last = std::sync::Arc::downgrade(&replacement);
    component.update_content(replacement);
    assert!(old.upgrade().is_none());
    assert!(last.upgrade().is_some());
    drop(component);
    assert!(last.upgrade().is_none());
}
