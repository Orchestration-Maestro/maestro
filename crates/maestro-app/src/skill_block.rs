//! The skill block a prompt carries when a skill command expands.
use regex::Regex;
use std::sync::LazyLock;

/// A skill block and the user text after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSkillBlock {
    /// The skill name.
    pub name: String,
    /// The skill file.
    pub location: String,
    /// The skill body.
    pub content: String,
    /// The trimmed user text after the block, absent when empty.
    pub user_message: Option<String>,
}

/// The complete block: header line, body, closing line and optional user text.
static SKILL_BLOCK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r#"^<skill name="([^"]+)" location="([^"]+)">\n([\s\S]*?)\n</skill>(?:\n\n([\s\S]+))?$"#,
    )
    .ok()
});

/// Parses a message that is exactly one skill block, optionally followed by user text.
#[must_use]
pub fn parse_skill_block(text: &str) -> Option<ParsedSkillBlock> {
    let captures = SKILL_BLOCK.as_ref()?.captures(text)?;
    let group = |index| captures.get(index).map_or("", |m| m.as_str()).to_owned();
    let user_message = captures
        .get(4)
        .map(|m| m.as_str().trim_matches(maestro_text::is_whitespace_scalar))
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    Some(ParsedSkillBlock {
        name: group(1),
        location: group(2),
        content: group(3),
        user_message,
    })
}
