//! Selectable prompt and available-tool details.
use super::document::invalid;
use super::header::{HeaderState, literal, text};
use crate::browser::attachment_content::{color, points};
use maestro_app::{canonical_entries, is_truthy, normalize_to_lf};
use maestro_theme::BrandPresentation;
use serde_json::{Map, Value};
use std::io;

/// Draw the optional prompt and available tools without changing raw data.
pub(super) fn render_details(
    ui: &mut egui::Ui,
    data: &Map<String, Value>,
    state: &mut HeaderState,
    p: &BrandPresentation<'_>,
) -> io::Result<()> {
    let prompt = literal(data.get("systemPrompt").unwrap_or(&Value::Null))?;
    if !prompt.is_empty() {
        render_prompt(ui, prompt, &mut state.prompt, p)?;
    }
    let tools = data.get("tools").unwrap_or(&Value::Null);
    if let Some(tools) = tools.as_array().filter(|t| !t.is_empty()) {
        text(ui, p, "Available Tools", "accent")?;
        for (position, tool) in tools.iter().enumerate() {
            let expanded = state.tools.contains(&position);
            let activate = ui
                .push_id(("header-tool", position), |ui| {
                    render_tool(ui, tool, expanded, p)
                })
                .inner?;
            if activate && !state.tools.remove(&position) {
                state.tools.insert(position);
            }
        }
    } else if tools.as_str().is_some_and(|s| !s.is_empty())
        || tools["length"].as_f64().is_some_and(|n| n > 0.)
    {
        return Err(invalid("nonarray tools"));
    }
    Ok(())
}

/// Use the supplied panel, border, spacing and radius tokens.
fn panel(p: &BrandPresentation<'_>) -> io::Result<egui::Frame> {
    Ok(egui::Frame::new()
        .fill(color(p, "panel")?)
        .stroke(egui::Stroke::new(1., color(p, "border")?))
        .corner_radius(points(p.radii["panel"]))
        .inner_margin(points(p.spacing["sm"])))
}
/// Choose the original LF preview before normalizing the selected display text.
fn render_prompt(
    ui: &mut egui::Ui,
    prompt: &str,
    expanded: &mut bool,
    p: &BrandPresentation<'_>,
) -> io::Result<()> {
    let lines: Vec<_> = prompt.split('\n').collect();
    let expandable = lines.len() > 10;
    let preview = if expandable && !*expanded {
        lines[..10].join("\n")
    } else {
        prompt.into()
    };
    let activate = panel(p)?
        .show(ui, |ui| -> io::Result<bool> {
            let mut activate = text(ui, p, "System Prompt", "accent")?;
            let body = egui::RichText::new(normalize_to_lf(&preview)).monospace();
            activate |= text(ui, p, body, "text")?;
            if expandable && !*expanded {
                let hint = format!("... ({} more lines, click to expand)", lines.len() - 10);
                activate |= text(ui, p, egui::RichText::new(hint).italics(), "muted")?;
            }
            Ok(activate)
        })
        .inner?;
    if expandable && activate {
        *expanded = !*expanded;
    }
    Ok(())
}

/// Render one positional tool, validating reached parameters even while collapsed.
fn render_tool(
    ui: &mut egui::Ui,
    tool: &Value,
    expanded: bool,
    p: &BrandPresentation<'_>,
) -> io::Result<bool> {
    if tool.is_null() {
        return Err(invalid("null tool"));
    }
    let properties = tool["parameters"]["properties"]
        .as_object()
        .filter(|m| !m.is_empty());
    let hint = if expanded {
        "[hide parameters]"
    } else {
        "[click to show parameters]"
    };
    panel(p)?
        .show(ui, |ui| {
            let mut activate = ui
                .horizontal_wrapped(|ui| -> io::Result<bool> {
                    let mut activate = text(ui, p, literal(&tool["name"])?, "text")?;
                    activate |= text(ui, p, " - ", "muted")?;
                    activate |= text(ui, p, literal(&tool["description"])?, "muted")?;
                    if properties.is_some() {
                        activate |= text(ui, p, hint, "muted")?;
                    }
                    Ok(activate)
                })
                .inner?;
            if let Some(properties) = properties {
                for property in canonical_entries(properties) {
                    activate |=
                        parameter(ui, p, property, &tool["parameters"]["required"], expanded)?;
                }
            }
            Ok(activate && properties.is_some())
        })
        .inner
}
/// Validate a reached schema property and optionally paint its literal labels.
fn parameter(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    (name, prop): (&String, &Value),
    required: &Value,
    expanded: bool,
) -> io::Result<bool> {
    if prop.is_null() {
        return Err(invalid("null schema property"));
    }
    let required = if is_truthy(required) {
        required
            .as_array()
            .ok_or_else(|| invalid("invalid required collection"))?
            .iter()
            .any(|v| v.as_str() == Some(name))
    } else {
        false
    };
    let kind = &prop["type"];
    let kind = if !is_truthy(kind) {
        "any".into()
    } else if let Some(types) = kind.as_array() {
        types
            .iter()
            .map(|v| v.as_str().ok_or_else(|| invalid("nonstring schema type")))
            .collect::<io::Result<Vec<_>>>()?
            .join(",")
    } else {
        literal(kind)?.to_owned()
    };
    let description = literal(&prop["description"])?;
    if !expanded {
        return Ok(false);
    }
    let mut activate = ui
        .horizontal_wrapped(|ui| -> io::Result<bool> {
            let mut activate = text(ui, p, egui::RichText::new(name).monospace(), "text")?;
            activate |= text(ui, p, &kind, "muted")?;
            let (label, role) = if required {
                ("required", "warning")
            } else {
                ("optional", "muted")
            };
            activate |= text(ui, p, label, role)?;
            Ok(activate)
        })
        .inner?;
    if !description.is_empty() {
        activate |= text(ui, p, description, "muted")?;
    }
    Ok(activate)
}
