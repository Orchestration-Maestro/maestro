//! Parsing for outer terminal window-title templates.

/// Maximum Unicode scalar values in a sanitized outer terminal window title.
const MAX_WINDOW_TITLE_CHARS: usize = 200;

/// Removes controls, limits to 200 Unicode scalars, then trims surrounding whitespace.
#[must_use]
pub fn sanitize_window_title_text(value: &str) -> Option<String> {
    let sanitized = value
        .chars()
        .filter(|ch| !matches!(*ch, '\u{1b}' | '\u{7}' | '\u{9c}') && !ch.is_control())
        .take(MAX_WINDOW_TITLE_CHARS)
        .collect::<String>()
        .trim()
        .to_string();
    (!sanitized.is_empty()).then_some(sanitized)
}

/// Returns the default outer terminal window-title format.
#[must_use]
pub fn default_window_title() -> String {
    "{hostname}: {workspace}".to_string()
}

/// Returns a configuration diagnostic for an invalid window-title template.
#[must_use]
pub fn window_title_diagnostics(template: &str) -> Option<String> {
    WindowTitleTemplate::parse(template)
        .err()
        .map(|err| format!("multiplexer.windowTitle {err}; leaving the outer terminal title alone"))
}

/// A value the daemon can substitute into a window title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTitleToken {
    /// Host the daemon runs on.
    Hostname,
    /// Active workspace display name.
    Workspace,
    /// Active tab display name.
    Tab,
    /// Manual name of the focused pane.
    Pane,
    /// Focused pane's terminal title, with spinner frames stripped.
    TerminalTitle,
}

impl WindowTitleToken {
    /// Consumes a token body through its closing brace.
    fn read(chars: &mut impl Iterator<Item = char>) -> Result<Self, String> {
        let mut name = String::new();
        for ch in chars {
            if ch == '}' {
                return Self::parse(name.trim())
                    .ok_or_else(|| format!("has unknown token '{{{name}}}'"));
            }
            name.push(ch);
        }
        Err("has an unclosed '{'".to_string())
    }

    /// Recognizes the supported template token names.
    fn parse(name: &str) -> Option<Self> {
        match name {
            "hostname" => Some(Self::Hostname),
            "workspace" => Some(Self::Workspace),
            "tab" => Some(Self::Tab),
            "pane" => Some(Self::Pane),
            "terminal_title" => Some(Self::TerminalTitle),
            _ => None,
        }
    }
}

/// One literal or substitution in a parsed window title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowTitlePart {
    /// Text emitted without substitution.
    Literal(String),
    /// A value supplied by the daemon.
    Token(WindowTitleToken),
}

/// Appends a nonempty accumulated literal and clears its buffer.
fn push_literal(parts: &mut Vec<WindowTitlePart>, literal: &mut String) {
    if !literal.is_empty() {
        parts.push(WindowTitlePart::Literal(std::mem::take(literal)));
    }
}

/// Parsed window-title format string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTitleTemplate {
    /// Literal and substitution parts in authored order.
    parts: Vec<WindowTitlePart>,
}

impl WindowTitleTemplate {
    /// Parses a format; an empty format disables outer window titles.
    ///
    /// # Errors
    /// Returns a diagnostic for unknown tokens or unbalanced braces.
    pub fn parse(template: &str) -> Result<Option<Self>, String> {
        if template.is_empty() {
            return Ok(None);
        }
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    literal.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    literal.push('}');
                }
                '{' => {
                    let token = WindowTitleToken::read(&mut chars)?;
                    push_literal(&mut parts, &mut literal);
                    parts.push(WindowTitlePart::Token(token));
                }
                '}' => return Err("has an unmatched '}'".to_string()),
                _ => literal.push(ch),
            }
        }
        push_literal(&mut parts, &mut literal);
        Ok((!parts.is_empty()).then_some(Self { parts }))
    }

    /// Borrows the parsed parts in authored order.
    #[must_use]
    pub fn parts(&self) -> &[WindowTitlePart] {
        &self.parts
    }

    /// Reports whether the format substitutes the given value.
    #[must_use]
    pub fn uses(&self, token: WindowTitleToken) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, WindowTitlePart::Token(other) if *other == token))
    }
}
