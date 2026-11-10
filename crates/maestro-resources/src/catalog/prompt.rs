//! Model-facing instruction resource formatting.
use super::Skill;
/// Format visible skills in caller order, using their instruction file locations.
#[must_use]
pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    let visible: Vec<_> = skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .collect();
    if visible.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "\n\nThese skills hold specialized instructions for particular tasks.".into(),
        "When a task fits a skill's description, load that skill's file with the read tool.".into(),
        "If a skill file mentions a relative path, resolve it from the skill's directory (the parent of SKILL.md, i.e. the dirname of its path) and pass the resulting absolute path to tool commands.".into(),
        String::new(),
        "<available_skills>".into(),
    ];
    for skill in visible {
        lines.push("  <skill>".into());
        lines.push(format!("    <name>{}</name>", escape_xml(&skill.name)));
        lines.push(format!(
            "    <description>{}</description>",
            escape_xml(&skill.description)
        ));
        lines.push(format!(
            "    <location>{}</location>",
            escape_xml(&skill.file_path)
        ));
        lines.push("  </skill>".into());
    }
    lines.push("</available_skills>".into());
    lines.join("\n")
}

/// Escape each input character once without revisiting generated entities.
fn escape_xml(value: &str) -> String {
    maestro_text::markup::escape(value, maestro_text::markup::EscapeMode::Xml)
}
