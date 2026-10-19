//! Sidebar row composition and occurrence-local styling.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use maestro_detection::agents::{Agent, agent_label, parse_canonical_agent_label};

/// Maximum rows in a sidebar layout.
const MAX_SIDEBAR_ROWS: usize = 16;
/// Maximum tokens in a sidebar row.
const MAX_SIDEBAR_TOKENS_PER_ROW: usize = 16;
/// Empty terminal rows between composed rows.
const DEFAULT_SIDEBAR_ROW_GAP: u16 = 0;

/// Decode rows and validate both layout limits.
fn deserialize_sidebar_rows<'de, D, T>(deserializer: D) -> Result<Vec<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    let rows = Vec::<Vec<T>>::deserialize(deserializer)?;
    validate_sidebar_rows(&rows).map_err(serde::de::Error::custom)?;
    Ok(rows)
}

/// Reject layouts exceeding the row or token limits.
fn validate_sidebar_rows<T>(rows: &[Vec<T>]) -> Result<(), String> {
    if rows.len() > MAX_SIDEBAR_ROWS {
        return Err(format!(
            "sidebar layouts may contain at most {MAX_SIDEBAR_ROWS} rows"
        ));
    }
    if rows
        .iter()
        .any(|row| row.len() > MAX_SIDEBAR_TOKENS_PER_ROW)
    {
        return Err(format!(
            "sidebar rows may contain at most {MAX_SIDEBAR_TOKENS_PER_ROW} tokens"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Authored RGB foreground color.
pub struct SidebarTokenColor {
    /// Red component.
    r: u8,
    /// Green component.
    g: u8,
    /// Blue component.
    b: u8,
}

impl Serialize for SidebarTokenColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b))
    }
}

impl<'de> Deserialize<'de> for SidebarTokenColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let hex = value.strip_prefix('#').filter(|hex| {
            hex.is_ascii()
                && matches!(hex.len(), 3 | 6)
                && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        let Some(hex) = hex else {
            return Err(serde::de::Error::custom(
                "sidebar token fg must be #RGB or #RRGGBB",
            ));
        };
        let expanded = if hex.len() == 3 {
            hex.chars()
                .flat_map(|digit| [digit, digit])
                .collect::<String>()
        } else {
            hex.to_owned()
        };
        let component =
            |range| u8::from_str_radix(&expanded[range], 16).map_err(serde::de::Error::custom);
        let (r, g, b) = (component(0..2)?, component(2..4)?, component(4..6)?);
        Ok(Self { r, g, b })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Optional style overrides for one token occurrence.
pub struct SidebarTokenStyle {
    /// Foreground color override.
    pub fg: Option<SidebarTokenColor>,
    /// Bold attribute override, including explicit false.
    pub bold: Option<bool>,
    /// Dim attribute override, including explicit false.
    pub dim: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Content available in an agent sidebar row.
pub enum AgentSidebarToken {
    /// Agent state icon.
    StateIcon,
    /// Agent state text.
    StateText,
    /// Workspace.
    Workspace,
    /// Tab.
    Tab,
    /// Pane.
    Pane,
    /// Agent.
    Agent,
    /// Full terminal title.
    TerminalTitle,
    /// Terminal title without agent decorations.
    TerminalTitleStripped,
    /// Named custom token, without its dollar prefix.
    Custom(String),
    /// Token with occurrence-local style overrides.
    Styled {
        /// Underlying token.
        token: Box<AgentSidebarToken>,
        /// Occurrence-local style.
        style: SidebarTokenStyle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Content available in a workspace sidebar row.
pub enum SpaceSidebarToken {
    /// Agent state icon.
    StateIcon,
    /// Agent state text.
    StateText,
    /// Workspace.
    Workspace,
    /// Branch.
    Branch,
    /// Git working-tree status.
    GitStatus,
    /// Named custom token, without its dollar prefix.
    Custom(String),
    /// Token with occurrence-local style overrides.
    Styled {
        /// Underlying token.
        token: Box<SpaceSidebarToken>,
        /// Occurrence-local style.
        style: SidebarTokenStyle,
    },
}

impl AgentSidebarToken {
    /// Underlying token and occurrence-local style.
    #[must_use]
    pub fn parts(&self) -> (&Self, SidebarTokenStyle) {
        match self {
            Self::Styled { token, style } => (token, *style),
            token => (token, SidebarTokenStyle::default()),
        }
    }
}

impl SpaceSidebarToken {
    /// Underlying token and occurrence-local style.
    #[must_use]
    pub fn parts(&self) -> (&Self, SidebarTokenStyle) {
        match self {
            Self::Styled { token, style } => (token, *style),
            token => (token, SidebarTokenStyle::default()),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict authored styled-token object.
struct RawStyledSidebarToken {
    /// Underlying token.
    token: String,
    #[serde(default)]
    /// Foreground color override.
    fg: Option<SidebarTokenColor>,
    #[serde(default)]
    /// Bold attribute override, including explicit false.
    bold: Option<bool>,
    #[serde(default)]
    /// Dim attribute override, including explicit false.
    dim: Option<bool>,
}

/// Plain or styled authored token.
enum RawSidebarToken {
    /// Unstyled token name.
    Plain(String),
    /// Token name with occurrence styling.
    Styled(RawStyledSidebarToken),
}

impl<'de> Deserialize<'de> for RawSidebarToken {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TokenVisitor)
    }
}

/// Admit token strings and styled objects, never positional record arrays.
struct TokenVisitor;

impl<'de> serde::de::Visitor<'de> for TokenVisitor {
    type Value = RawSidebarToken;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a token string or styled token object")
    }

    fn visit_str<E: serde::de::Error>(self, token: &str) -> Result<Self::Value, E> {
        Ok(RawSidebarToken::Plain(token.to_owned()))
    }

    fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
        RawStyledSidebarToken::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            .map(RawSidebarToken::Styled)
    }
}

impl RawSidebarToken {
    /// Underlying token and occurrence-local style.
    fn parts(self) -> (String, Option<SidebarTokenStyle>) {
        match self {
            Self::Plain(token) => (token, None),
            Self::Styled(token) => (
                token.token,
                Some(SidebarTokenStyle {
                    fg: token.fg,
                    bold: token.bold,
                    dim: token.dim,
                }),
            ),
        }
    }
}

/// Resolve a built-in name or validate a custom token identifier.
fn parse_sidebar_token<T>(value: &str, builtins: &[(&str, T)]) -> Result<T, String>
where
    T: Clone + From<String>,
{
    if let Some((_, token)) = builtins.iter().find(|(name, _)| *name == value) {
        return Ok(token.clone());
    }
    let Some(name) = value.strip_prefix('$') else {
        return Err(format!(
            "unknown sidebar token `{value}`; custom tokens must start with `$`"
        ));
    };
    if name.is_empty()
        || name.len() > 32
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return Err(format!("invalid custom sidebar token `{value}`"));
    }
    Ok(T::from(name.to_string()))
}

/// Encode a styled occurrence without dropping explicit false attributes.
fn serialize_styled_token<S>(
    name: &str,
    style: SidebarTokenStyle,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry("token", &name)?;
    if let Some(fg) = style.fg {
        map.serialize_entry("fg", &fg)?;
    }
    if let Some(bold) = style.bold {
        map.serialize_entry("bold", &bold)?;
    }
    if let Some(dim) = style.dim {
        map.serialize_entry("dim", &dim)?;
    }
    map.end()
}

/// Stored name of an agent token.
fn agent_token_name(token: &AgentSidebarToken) -> String {
    match token {
        AgentSidebarToken::StateIcon => "state_icon".into(),
        AgentSidebarToken::StateText => "state_text".into(),
        AgentSidebarToken::Workspace => "workspace".into(),
        AgentSidebarToken::Tab => "tab".into(),
        AgentSidebarToken::Pane => "pane".into(),
        AgentSidebarToken::Agent => "agent".into(),
        AgentSidebarToken::TerminalTitle => "terminal_title".into(),
        AgentSidebarToken::TerminalTitleStripped => "terminal_title_stripped".into(),
        AgentSidebarToken::Custom(name) => format!("${name}"),
        AgentSidebarToken::Styled { token, .. } => agent_token_name(token),
    }
}

/// Stored name of a workspace token.
fn space_token_name(token: &SpaceSidebarToken) -> String {
    match token {
        SpaceSidebarToken::StateIcon => "state_icon".into(),
        SpaceSidebarToken::StateText => "state_text".into(),
        SpaceSidebarToken::Workspace => "workspace".into(),
        SpaceSidebarToken::Branch => "branch".into(),
        SpaceSidebarToken::GitStatus => "git_status".into(),
        SpaceSidebarToken::Custom(name) => format!("${name}"),
        SpaceSidebarToken::Styled { token, .. } => space_token_name(token),
    }
}

impl Serialize for AgentSidebarToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Styled { token, style } => {
                serialize_styled_token(&agent_token_name(token), *style, serializer)
            }
            token => serializer.serialize_str(&agent_token_name(token)),
        }
    }
}

impl From<String> for AgentSidebarToken {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

impl<'de> Deserialize<'de> for AgentSidebarToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (value, style) = RawSidebarToken::deserialize(deserializer)?.parts();
        let token = parse_sidebar_token(
            &value,
            &[
                ("state_icon", Self::StateIcon),
                ("state_text", Self::StateText),
                ("workspace", Self::Workspace),
                ("tab", Self::Tab),
                ("pane", Self::Pane),
                ("agent", Self::Agent),
                ("terminal_title", Self::TerminalTitle),
                ("terminal_title_stripped", Self::TerminalTitleStripped),
            ],
        )
        .map_err(serde::de::Error::custom)?;
        Ok(match style {
            Some(style) => Self::Styled {
                token: Box::new(token),
                style,
            },
            None => token,
        })
    }
}

impl Serialize for SpaceSidebarToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Styled { token, style } => {
                serialize_styled_token(&space_token_name(token), *style, serializer)
            }
            token => serializer.serialize_str(&space_token_name(token)),
        }
    }
}

impl From<String> for SpaceSidebarToken {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

impl<'de> Deserialize<'de> for SpaceSidebarToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (value, style) = RawSidebarToken::deserialize(deserializer)?.parts();
        let token = parse_sidebar_token(
            &value,
            &[
                ("state_icon", Self::StateIcon),
                ("state_text", Self::StateText),
                ("workspace", Self::Workspace),
                ("branch", Self::Branch),
                ("git_status", Self::GitStatus),
            ],
        )
        .map_err(serde::de::Error::custom)?;
        Ok(match style {
            Some(style) => Self::Styled {
                token: Box::new(token),
                style,
            },
            None => token,
        })
    }
}

/// Ordered agent sidebar rows.
type AgentSidebarRows = Vec<Vec<AgentSidebarToken>>;
/// Ordered workspace sidebar rows.
type SpaceSidebarRows = Vec<Vec<SpaceSidebarToken>>;

/// Validate canonical agent keys and each override layout.
fn deserialize_rows_by_agent<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, AgentSidebarRows>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let rows_by_agent = BTreeMap::<String, AgentSidebarRows>::deserialize(deserializer)?;
    for (id, rows) in &rows_by_agent {
        if parse_canonical_agent_label(id).is_none() {
            return Err(serde::de::Error::custom(format!(
                "unknown canonical agent id `{id}` in sidebar rowsByAgent"
            )));
        }
        validate_sidebar_rows(rows).map_err(serde::de::Error::custom)?;
    }
    Ok(rows_by_agent)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
/// Agent sidebar layout with optional per-agent overrides.
pub struct AgentsSidebarConfig {
    #[serde(deserialize_with = "deserialize_sidebar_rows")]
    /// Ordered token rows.
    pub rows: AgentSidebarRows,
    #[serde(default, deserialize_with = "deserialize_rows_by_agent")]
    /// Layouts keyed by canonical detection agent names.
    pub rows_by_agent: BTreeMap<String, AgentSidebarRows>,
    /// Empty terminal rows between token rows.
    pub row_gap: u16,
}

impl AgentsSidebarConfig {
    /// Agent override, or the shared layout when no override exists.
    #[must_use]
    pub fn rows_for_agent(&self, agent: Option<Agent>) -> &AgentSidebarRows {
        agent
            .and_then(|agent| self.rows_by_agent.get(agent_label(agent)))
            .unwrap_or(&self.rows)
    }
}

impl Default for AgentsSidebarConfig {
    fn default() -> Self {
        Self {
            rows: vec![
                vec![
                    AgentSidebarToken::StateIcon,
                    AgentSidebarToken::Workspace,
                    AgentSidebarToken::Tab,
                ],
                vec![AgentSidebarToken::Agent],
            ],
            rows_by_agent: BTreeMap::new(),
            row_gap: DEFAULT_SIDEBAR_ROW_GAP,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
/// Workspace sidebar layout.
pub struct SpacesSidebarConfig {
    #[serde(deserialize_with = "deserialize_sidebar_rows")]
    /// Ordered token rows.
    pub rows: SpaceSidebarRows,
    /// Empty terminal rows between token rows.
    pub row_gap: u16,
}

impl Default for SpacesSidebarConfig {
    fn default() -> Self {
        Self {
            rows: vec![
                vec![SpaceSidebarToken::StateIcon, SpaceSidebarToken::Workspace],
                vec![SpaceSidebarToken::Branch, SpaceSidebarToken::GitStatus],
            ],
            row_gap: DEFAULT_SIDEBAR_ROW_GAP,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
/// Independent agent and workspace sidebar layouts.
pub struct SidebarConfig {
    /// Agent sidebar preferences.
    pub agents: AgentsSidebarConfig,
    /// Workspace sidebar preferences.
    pub spaces: SpacesSidebarConfig,
}
