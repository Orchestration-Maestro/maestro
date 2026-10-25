//! Summary emission and unchanged authored user styling through public widgets.
use super::{
    REGISTRY, Restore, TERMINAL_CONTROL_CASES, Widget, bind, markdown, palette, published,
};
use maestro_tui::{Component, TerminalCapabilities, TerminalImage, get_keybindings};
use std::rc::Rc;

/// Constructs either public summary with the same controlled rendering inputs.
fn summary(kind: &str, text: &str, hyperlinks: bool) -> Widget {
    let (_, live) = published::published(&palette::palette("a"));
    let markdown = Rc::new(markdown(live.clone(), true));
    let terminal = TerminalImage::new(|_| None, || 1);
    terminal.set_capabilities(TerminalCapabilities {
        hyperlinks,
        ..Default::default()
    });
    let input = serde_json::from_value(serde_json::json!({
        "kind": kind, "text": text, "width": 160, "tokens": 12345,
        "environment": {"LANG": "en_US.UTF-8"},
    }))
    .unwrap();
    Widget::with_terminal(&input, live, markdown, terminal).0
}

/// Observes each retained-state transition separately, including same-state rebuilds.
fn frames(widget: &Widget) -> Vec<String> {
    let mut frames = vec![widget.render(160).join("\n")];
    widget.expand(true);
    frames.push(widget.render(160).join("\n"));
    widget.expand(true);
    frames.push(widget.render(160).join("\n"));
    widget.invalidate();
    frames.push(widget.render(160).join("\n"));
    widget.expand(false);
    frames.push(widget.render(160).join("\n"));
    widget.expand(true);
    frames.push(widget.render(160).join("\n"));
    frames
}

/// Checks authored controls without stripping renderer-generated terminal sequences.
fn assert_clean(rows: &str, name: &str) {
    for forbidden in [
        "\x1b]52;",
        "\x1b]0;",
        "\x1b[2J",
        "attacker.example",
        "\x01",
        "\x08",
        "\x0b",
        "\x1f",
        "\u{fff9}",
        "\u{fffa}",
        "\u{fffb}",
    ] {
        assert!(
            !rows.contains(forbidden),
            "{name}: authored control remains"
        );
    }
}

/// Checks source-backed case-specific content and renderer-owned output.
fn assert_body(rows: &str, name: &str, hyperlinks: bool) {
    let prose = rows
        .split_once("before ")
        .map_or("", |(_, body)| body.split_once(" after").unwrap().0);
    match name {
        "safe styles" => {
            assert!(
                rows.lines()
                    .any(|line| line.contains("before") && line.contains("\x1b[1m")),
                "{name}: body bold lost"
            );
            assert_destination(prose, "https://safe.example", name, hyperlinks);
        }
        "entity destination" => {
            assert_destination(prose, "https://safe.example/", name, hyperlinks);
        }
        "entity label" => assert_destination(prose, "https://safe.example", name, hyperlinks),
        "numeric entity" => assert!(
            rows.contains("code &#27;]52;c;b3duZWQ=&#7; end"),
            "{name}: code entities changed"
        ),
        "split entity" => assert!(
            prose.contains("&#27;]52;c;b3duZWQ="),
            "{name}: reparsed entity"
        ),
        "link" => assert_eq!(rows.matches("docs.example").count(), 2, "{name}"),
        "title" => assert!(!rows.contains("owned"), "{name}: command payload displayed"),
        "binary" => assert_eq!(rows.matches("leftright").count(), 2, "{name}"),
        "unicode" => assert_eq!(rows.matches("café 中文 👩‍💻").count(), 2, "{name}"),
        "whitespace" => assert_payload_lines(rows, name),
        _ => {}
    }
}

/// Checks payload row boundaries independently of headers, spacers and fences.
fn assert_payload_lines(rows: &str, name: &str) {
    for word in ["tab", "value", "line", "next"] {
        assert_eq!(rows.matches(word).count(), 2, "{name}: {word}");
    }
    let lines: Vec<_> = rows.lines().collect();
    for (prefix, suffix) in [("before tab", "next after"), ("code tab", "next end")] {
        let start = lines.iter().position(|line| line.contains(prefix)).unwrap();
        assert!(lines[start].contains("value"), "{name}: tab value row");
        assert!(
            !lines[start].contains("line"),
            "{name}: first boundary lost"
        );
        assert!(lines[start + 1].contains("line"), "{name}: line row");
        assert!(
            !lines[start + 1].contains("next"),
            "{name}: second boundary lost"
        );
        assert!(lines[start + 2].contains(suffix), "{name}: next row");
    }
}

/// Inspects the actual generated destination in both capability branches.
fn assert_destination(rows: &str, destination: &str, name: &str, hyperlinks: bool) {
    if hyperlinks {
        assert!(
            rows.contains(&format!("\x1b]8;;{destination}\x1b\\")),
            "{name}: generated destination lost"
        );
    } else {
        assert!(
            rows.contains(&format!("({destination})")),
            "{name}: printed destination lost"
        );
        assert!(
            !rows.contains("\x1b]8;;"),
            "{name}: unexpected generated hyperlink"
        );
    }
}

/// Replays sixteen inputs and six frames independently for the selected summary kind.
pub fn summary_controls(kind: &str) {
    let _serial = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    bind(vec!["ctrl+o".into()]);
    let inputs = TERMINAL_CONTROL_CASES
        .iter()
        .map(|&(name, payload)| (name, payload, true))
        .chain([("empty", "", true)])
        .chain(
            TERMINAL_CONTROL_CASES
                .iter()
                .filter(|(name, _)| matches!(*name, "safe styles" | "entity destination"))
                .map(|&(name, payload)| (name, payload, false)),
        );
    let mut count = 0;
    for (name, payload, hyperlinks) in inputs {
        count += 1;
        let text = if payload.is_empty() {
            String::new()
        } else {
            format!("before {payload} after\n\n```\ncode {payload} end\n```")
        };
        let frames = frames(&summary(kind, &text, hyperlinks));
        assert_eq!(frames[0], frames[4], "{kind} {name}: collapsed");
        for index in [2, 3, 5] {
            assert_eq!(frames[1], frames[index], "{kind} {name}: expanded {index}");
        }
        for (index, rows) in frames.iter().enumerate() {
            assert_clean(rows, name);
            assert!(rows.contains(&format!("[{kind}]")), "{kind} {name}: label");
            if matches!(index, 0 | 4) {
                assert!(rows.contains("to expand)"), "{kind} {name}: hint");
                assert!(!rows.contains("before"), "{kind} {name}: collapsed body");
            } else {
                assert_expanded(rows, kind, name, payload, hyperlinks);
            }
        }
    }
    assert_eq!(count, 16);
}

/// Checks the expanded header and each input's own surrounding words.
fn assert_expanded(rows: &str, kind: &str, name: &str, payload: &str, hyperlinks: bool) {
    let header = if kind == "branch" {
        "Branch Summary"
    } else {
        "Compacted from 12,345 tokens"
    };
    assert!(rows.contains(header), "{kind} {name}: header");
    for word in ["before", "after", "code", "end"] {
        assert_eq!(
            rows.matches(word).count(),
            usize::from(!payload.is_empty()),
            "{kind} {name}: {word}"
        );
    }
    assert_body(rows, name, hyperlinks);
}

/// User-authored colour and prompt markers remain outside model-content cleaning.
pub fn user_styles() {
    let (_, live) = published::published(&palette::palette("a"));
    let component = maestro_chat::UserMessageComponent::new(
        "before \x1b[31mred\x1b[0m after".into(),
        live.clone(),
        Rc::new(markdown(live, true)),
        TerminalImage::new(|_| None, || 1),
    );
    let rows = component.render(160).join("\n");
    assert!(rows.contains("\x1b[31mred\x1b[0m"));
    for word in ["before", "after"] {
        assert_eq!(rows.matches(word).count(), 1);
    }
    for marker in ["\x1b]133;A\x07", "\x1b]133;B\x07", "\x1b]133;C\x07"] {
        assert!(rows.contains(marker));
    }
}
