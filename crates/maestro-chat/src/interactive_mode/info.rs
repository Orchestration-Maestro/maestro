//! The session name and session information commands.
use super::InteractiveMode;
use maestro_app::{
    LocaleNumberFormat, SessionStats, UsageCostBreakdownEntry, is_whitespace_scalar,
};
use maestro_theme::{Theme, ThemeColor};
use std::{error::Error, fmt::Write as _};

impl InteractiveMode {
    /// Shows the current session name for `/name` alone, or a usage warning without one;
    /// otherwise records the supplied name, trimmed, and confirms it.
    ///
    /// # Errors
    /// Returns the failure to read or record the name, or of the published theme's colours.
    pub fn handle_name_command(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let name = text
            .strip_prefix("/name")
            .unwrap_or(text)
            .trim_matches(is_whitespace_scalar);
        let session = self.runtime_host.session();
        if name.is_empty() {
            return match session.session_name()? {
                Some(current) => {
                    Ok(self.notice(&ThemeColor::Dim, &format!("Session name: {current}"))?)
                }
                None => Ok(self.show_warning("Usage: /name <name>")?),
            };
        }
        session.set_session_name(name)?;
        Ok(self.notice(&ThemeColor::Dim, &format!("Session name set: {name}"))?)
    }

    /// Shows the current session's name, file, identity, message counts, token totals and
    /// positive cost.
    ///
    /// # Errors
    /// Returns the failure to read the statistics or the name, to prepare the locale's
    /// number format, or of the published theme's colours.
    pub fn handle_session_command(&self) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let stats = session.get_session_stats()?;
        let buckets = session.get_usage_cost_breakdown()?;
        let name = session.session_name()?;
        let theme = self.themes.theme().get()?;
        let info = session_info(&theme, &stats, name.as_deref(), &buckets)?;
        self.add_text(info, 0);
        Ok(())
    }
}

/// The session information: bold headings, dim labels, token totals in the environment's
/// locale and a positive cost with four decimals, split by cost bucket when there are several.
///
/// # Errors
/// Returns the failure to prepare the locale's number format or to format a total, or of
/// the theme's dim colour.
pub(super) fn session_info(
    theme: &Theme,
    stats: &SessionStats,
    name: Option<&str>,
    buckets: &[UsageCostBreakdownEntry],
) -> Result<String, Box<dyn Error>> {
    let numbers = LocaleNumberFormat::new(&|key| std::env::var(key).ok())?;
    let label = |text| theme.fg(&ThemeColor::Dim, text);
    let line = |text, value: &str| Ok::<_, Box<dyn Error>>(format!("{} {value}\n", label(text)?));
    let heading = |text| Theme::bold(text, true);
    let mut info = format!("{}\n\n", heading("Session Info"));
    if let Some(name) = name {
        info += &line("Name:", name)?;
    }
    info += &line(
        "File:",
        stats.session_file.as_deref().unwrap_or("In-memory"),
    )?;
    info += &line("ID:", &stats.session_id)?;
    writeln!(info, "\n{}", heading("Messages"))?;
    for (text, count) in [
        ("User:", stats.user_messages),
        ("Assistant:", stats.assistant_messages),
        ("Tool Calls:", stats.tool_calls),
        ("Tool Results:", stats.tool_results),
        ("Total:", stats.total_messages),
    ] {
        info += &line(text, &count.to_string())?;
    }
    writeln!(info, "\n{}", heading("Tokens"))?;
    let tokens = &stats.tokens;
    for (text, value, shown) in [
        ("Input:", tokens.input, true),
        ("Output:", tokens.output, true),
        ("Cache Read:", tokens.cache_read, tokens.cache_read > 0.0),
        ("Cache Write:", tokens.cache_write, tokens.cache_write > 0.0),
        ("Total:", tokens.total, true),
    ] {
        if shown {
            info += &line(text, &numbers.format(value)?)?;
        }
    }
    if stats.cost > 0.0 {
        let fixed = |cost| ryu_js::Buffer::new().format_to_fixed(cost, 4).to_owned();
        let cost = fixed(stats.cost);
        write!(info, "\n{}\n{} {cost}", heading("Cost"), label("Total:")?)?;
        if buckets.len() > 1 {
            for bucket in buckets {
                let tokens = numbers.format(bucket.tokens)?;
                let text = format!("{}:", bucket.key);
                let key = theme.fg(&ThemeColor::Dim, &text)?;
                write!(info, "\n  {key} {} ({tokens} tokens)", fixed(bucket.cost))?;
            }
        }
    }
    Ok(info)
}
