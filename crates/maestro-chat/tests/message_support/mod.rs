//! Strict recorded message constructor and operation replay.
mod input;
mod summaries;
use crate::palette;
use crate::published;
pub use summaries::{summary_controls, user_styles};
mod widget;
use crate::registry::REGISTRY;
use input::{Action, Case};
use maestro_theme::LiveTheme;
use maestro_tui::{Component, KeybindingsManager, MarkdownTheme, get_keybindings, set_keybindings};
use std::{collections::HashSet, rc::Rc};
use widget::Widget;

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
/// Uses the controlled palette with the real Markdown callbacks and highlighter.
fn markdown(live: LiveTheme, enabled: bool) -> MarkdownTheme {
    maestro_theme::get_markdown_theme(
        live,
        Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        enabled,
    )
}
/// Replays one complete recorded invocation, preserving frame order.
fn replay(case: &Case) {
    let input = &case.input;
    let (state, live) =
        published::published(&palette::palette(input.palette.as_deref().unwrap_or("a")));
    bind(vec!["ctrl+o".into()]);
    let enabled = input.styles_enabled.unwrap_or(true);
    let mut markdown = markdown(live.clone(), enabled);
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
            Action::Invalidate {} => widget.invalidate(),
            Action::Theme { value } => {
                state.set_theme_instance(palette::palette(value)).unwrap();
            }
            Action::Keys { value } => bind(value.clone()),
            Action::CaptureChildren {} => alias = Some(widget.children()),
            Action::Clear {} => widget.clear(),
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
    let markdown = Rc::new(markdown(live.clone(), true));
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

/// Literal and decoded attacks, with ordinary terminal styling as a control.
const TERMINAL_CONTROL_CASES: &[(&str, &str)] = &[
    ("clipboard", "\x1b]52;c;b3duZWQ=\x07"),
    ("whitespace", "tab\tvalue\r\nline\nnext"),
    (
        "binary",
        "left\x01\x08\x0b\x1f\u{fff9}\u{fffa}\u{fffb}right",
    ),
    ("unicode", "café 中文 👩‍💻"),
    ("safe styles", "**bold** [safe](https://safe.example)"),
    ("numeric entity", "&#27;]52;c;b3duZWQ=&#7;"),
    (
        "entity label",
        "[&#27;&#93;52;c;b3duZWQ=&#7;](https://safe.example)",
    ),
    (
        "entity destination",
        "[label](https://safe.example/&#27;]52;c;b3duZWQ=&#7;)",
    ),
    ("split entity", "&\x01#27;]52;c;b3duZWQ=&#7;"),
    ("code span", "`\x1b]52;c;b3duZWQ=\x07`"),
    ("title", "\x1b]0;owned\x07"),
    ("clear", "\x1b[2J"),
    (
        "link",
        "\x1b]8;;https://attacker.example/\x07docs.example\x1b]8;;\x07",
    ),
];

/// Text and thinking that carry terminal control sequences in prose and fenced code
/// render without them, keeping the words around them.
pub fn drops_terminal_controls() {
    let (_, live) = published::published(&palette::palette("a"));
    let markdown = Rc::new(markdown(live.clone(), true));
    let mut leaks = Vec::new();
    for &(name, control) in TERMINAL_CONTROL_CASES {
        let text = format!("before {control} after\n\n```\ncode {control} end\n```");
        let message = widget::message(None);
        message.write().unwrap().content = serde_json::from_value(serde_json::json!([
            {"type": "thinking", "thinking": text},
            {"type": "text", "text": text},
        ]))
        .unwrap();
        let terminal = maestro_tui::TerminalImage::new(|_| None, || 1);
        terminal.set_capabilities(maestro_tui::TerminalCapabilities {
            hyperlinks: true,
            ..Default::default()
        });
        let component = maestro_chat::AssistantMessageComponent::new(
            Some(message),
            maestro_chat::AssistantMessageOptions::default(),
            live.clone(),
            markdown.clone(),
            terminal,
        );
        let rendered = component.render(80).join("\n");
        if name == "safe styles" {
            assert!(rendered.contains("\x1b[1m"), "bold styling lost");
            assert!(
                rendered.contains("\x1b]8;;https://safe.example\x1b\\"),
                "hyperlink lost"
            );
        }
        let markers = ["\x1b]52;", "\x1b]0;", "\x1b[2J", "attacker.example"];
        leaks.extend(
            markers
                .iter()
                .filter(|marker| rendered.contains(**marker))
                .map(|marker| (name, *marker)),
        );
        for word in ["before", "after", "code", "end"] {
            assert_eq!(
                rendered.matches(word).count(),
                2,
                "{name} {word}: {rendered:?}"
            );
        }
    }
    assert!(leaks.is_empty(), "rendered controls: {leaks:?}");
}

/// The rows of an assistant message holding `content`, with thinking hidden or shown.
fn assistant_rows(content: serde_json::Value, hide: bool) -> Vec<String> {
    let (_, live) = published::published(&palette::palette("a"));
    let markdown = Rc::new(markdown(live.clone(), true));
    let message = widget::message(None);
    message.write().unwrap().content = serde_json::from_value(content).unwrap();
    let component = maestro_chat::AssistantMessageComponent::new(
        Some(message),
        maestro_chat::AssistantMessageOptions {
            hide_thinking_block: hide,
            hidden_thinking_label: "Thinking...".into(),
            styles_enabled: true,
        },
        live,
        markdown,
        maestro_tui::TerminalImage::new(|_| None, || 1),
    );
    component.render(40)
}

/// Adjacent thinking blocks render as one section; a tool call or any text splits them.
pub fn adjacent_thinking() {
    use serde_json::json;
    let thinking = |text: &str| json!({"type": "thinking", "thinking": text});
    let text = |text: &str| json!({"type": "text", "text": text});
    let tool = json!({"type": "toolCall", "id": "t", "name": "tool", "arguments": {}});
    let run = json!([
        thinking("first thought"),
        thinking(""),
        thinking("second thought"),
        text("answer")
    ]);
    assert_eq!(
        assistant_rows(run.clone(), true),
        assistant_rows(json!([thinking("first thought"), text("answer")]), true)
    );
    assert_eq!(
        assistant_rows(run, false),
        assistant_rows(
            json!([thinking("first thought\n\nsecond thought"), text("answer")]),
            false
        )
    );
    let labels = |content| {
        assistant_rows(content, true)
            .iter()
            .filter(|row| row.contains("Thinking..."))
            .count()
    };
    assert_eq!(labels(json!([thinking("a"), thinking("b")])), 1);
    assert_eq!(labels(json!([thinking("a"), tool, thinking("b")])), 2);
    assert_eq!(labels(json!([thinking("a"), text("  "), thinking("b")])), 2);
    assert_eq!(labels(json!([thinking(" "), thinking("")])), 0);
}

/// A user message is one Markdown layer that pads and colours itself.
pub fn user_markdown_layer() {
    let (_, live) = published::published(&palette::palette("a"));
    let markdown = Rc::new(markdown(live.clone(), true));
    let component = maestro_chat::UserMessageComponent::new(
        "hello".into(),
        live,
        markdown,
        maestro_tui::TerminalImage::new(|_| None, || 1),
    );
    let children = component.children().borrow().clone();
    assert_eq!(children.len(), 1);
    let child: Rc<dyn std::any::Any> = Rc::<dyn Component>::clone(&children[0]);
    assert!(child.downcast::<maestro_tui::Markdown>().is_ok());
    assert_eq!(component.render(20).len(), 3);
}
