//! Rendering of the complete command-line help document.

use super::super::pad_column;
use maestro_app::{
    APP_NAME, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, ExtensionFlag, FlagType,
};
use maestro_theme::Theme;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};

/// The complete help document with its substitution and heading markers.
const HELP_TEMPLATE: &str = include_str!("help.txt");

/// Prefix marking a line whose whole text is a bold heading.
const HEADING: &str = "~";

/// The code-unit column every extension flag label is padded to.
const LABEL_COLUMN: usize = 30;

/// The code-unit column the agent- and session-directory labels are padded to.
const ENV_COLUMN: usize = 32;

/// Render one retained registration as its label and description.
fn flag_row(flag: &ExtensionFlag) -> String {
    let value = if flag.r#type == FlagType::String {
        " <value>"
    } else {
        ""
    };
    let label = pad_column(&format!("  --{}{value}", flag.name), LABEL_COLUMN);
    match flag.description.as_deref() {
        Some(description) => format!("{label}{description}"),
        None => format!("{label}Registered by {}", flag.extension_path),
    }
}

/// Read every retained registration while building the flag section.
fn flag_section(flags: &[Arc<RwLock<ExtensionFlag>>], styles_enabled: bool) -> String {
    let rows: Vec<String> = flags
        .iter()
        .filter_map(|flag| flag.read().ok())
        .map(|flag| flag_row(&flag))
        .collect();
    if rows.is_empty() {
        return String::new();
    }
    format!(
        "\n{}\n{}\n",
        Theme::bold("Extension CLI Flags:", styles_enabled),
        rows.join("\n")
    )
}

/// Render the help body with its identity values and optional bold headings.
fn render(section: &str, styles_enabled: bool) -> String {
    let document = HELP_TEMPLATE
        .replace("%APP_BOLD%", &Theme::bold(APP_NAME, styles_enabled))
        .replace("%APP_UPPER%", &APP_NAME.to_uppercase())
        .replace("%APP%", APP_NAME)
        .replace("%CONFIG%", CONFIG_DIR_NAME)
        .replace("%ENV_AGENT%", &pad_column(&ENV_AGENT_DIR, ENV_COLUMN))
        .replace("%ENV_SESSION%", &pad_column(&ENV_SESSION_DIR, ENV_COLUMN));
    let mut rendered = String::with_capacity(document.len());
    for line in document.split_inclusive('\n') {
        let (line, heading) = match line.strip_prefix(HEADING) {
            Some(line) => (line, true),
            None => (line, false),
        };
        if !heading {
            rendered.push_str(line);
            continue;
        }
        let (body, end) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        rendered.push_str(&Theme::bold(body, styles_enabled));
        rendered.push_str(end);
    }
    rendered.replace(EXTENSION_MARKER, section)
}

/// The placeholder replaced by the optional extension flag section.
const EXTENSION_MARKER: &str = "%EXTENSION%";

/// Write the complete help document.
///
/// Every retained registration is read once while the section is built, and no
/// guard is held while writing.
///
/// # Errors
/// Returns the error of writing to `output`.
pub fn print_help(
    extension_flags: &[Arc<RwLock<ExtensionFlag>>],
    output: &mut dyn Write,
    styles_enabled: bool,
) -> io::Result<()> {
    let section = flag_section(extension_flags, styles_enabled);
    let document = render(&section, styles_enabled);
    output.write_all(document.as_bytes())?;
    output.write_all(b"\n")
}
