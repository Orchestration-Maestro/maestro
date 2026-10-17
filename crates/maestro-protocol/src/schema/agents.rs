//! Agents protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::{AgentStatus, ReadFormat, ReadSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent read params record.
#[schemars(description = "")]
pub struct AgentReadParams {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
    /// Source.
    #[schemars(description = "")]
    pub source: ReadSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Lines.
    #[schemars(description = "")]
    pub lines: Option<u32>,
    #[serde(default)]
    /// Format.
    #[schemars(description = "")]
    pub format: ReadFormat,
    #[serde(default = "super::common::default_true")]
    /// Strip ansi.
    #[schemars(description = "")]
    pub strip_ansi: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent send keys params record.
#[schemars(description = "")]
pub struct AgentSendKeysParams {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
    /// Keys.
    #[schemars(description = "")]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent wait params record.
#[schemars(description = "")]
pub struct AgentWaitParams {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Until.
    #[schemars(description = "")]
    pub until: Vec<AgentStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Timeout ms.
    #[schemars(description = "")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent prompt wait options record.
#[schemars(description = "")]
pub struct AgentPromptWaitOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Until.
    #[schemars(description = "")]
    pub until: Vec<AgentStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Timeout ms.
    #[schemars(description = "")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent rename params record.
#[schemars(description = "")]
pub struct AgentRenameParams {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Name.
    #[schemars(description = "")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent view set params record.
#[schemars(description = "")]
pub struct AgentViewSetParams {
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Filter.
    #[schemars(description = "")]
    pub filter: Option<AgentViewFilter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Sort.
    #[schemars(description = "")]
    pub sort: Vec<AgentViewSort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Agent view clear params record.
#[schemars(description = "")]
pub struct AgentViewClearParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source.
    #[schemars(description = "")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
/// Agent view filter record.
#[schemars(description = "")]
pub enum AgentViewFilter {
    /// All.
    #[schemars(description = "")]
    All {
        /// Filters.
        #[schemars(description = "")]
        filters: Vec<AgentViewFilter>,
    },
    /// Any.
    #[schemars(description = "")]
    Any {
        /// Filters.
        #[schemars(description = "")]
        filters: Vec<AgentViewFilter>,
    },
    /// Not.
    #[schemars(description = "")]
    Not {
        /// Filter.
        #[schemars(description = "")]
        filter: Box<AgentViewFilter>,
    },
    /// Eq.
    #[schemars(description = "")]
    Eq {
        /// Field.
        #[schemars(description = "")]
        field: AgentViewField,
        /// Value.
        #[schemars(description = "")]
        value: AgentViewValue,
    },
    /// In.
    #[schemars(description = "")]
    In {
        /// Field.
        #[schemars(description = "")]
        field: AgentViewField,
        /// Values.
        #[schemars(description = "")]
        values: Vec<AgentViewValue>,
    },
    /// Exists.
    #[schemars(description = "")]
    Exists {
        /// Field.
        #[schemars(description = "")]
        field: AgentViewField,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
/// Agent view field record.
#[schemars(description = "")]
pub enum AgentViewField {
    /// Builtin.
    #[schemars(description = "")]
    Builtin(AgentViewBuiltinField),
    /// Token.
    #[schemars(description = "")]
    Token {
        /// Token.
        #[schemars(description = "")]
        token: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Agent view builtin field record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentViewBuiltinField {
    /// Status.
    #[schemars(description = "")]
    Status,
    /// Workspace id.
    #[schemars(description = "")]
    WorkspaceId,
    /// Tab id.
    #[schemars(description = "")]
    TabId,
    /// Pane id.
    #[schemars(description = "")]
    PaneId,
    /// Agent.
    #[schemars(description = "")]
    Agent,
    /// Seen.
    #[schemars(description = "")]
    Seen,
    /// State change seq.
    #[schemars(description = "")]
    StateChangeSeq,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
/// Agent view value record.
#[schemars(description = "")]
pub enum AgentViewValue {
    /// String.
    #[schemars(description = "")]
    String(String),
    /// Bool.
    #[schemars(description = "")]
    Bool(bool),
    /// Number.
    #[schemars(description = "")]
    Number(u64),
    /// Context.
    #[schemars(description = "")]
    Context {
        /// Context.
        #[schemars(description = "")]
        context: AgentViewContext,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Agent view context record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentViewContext {
    /// Current workspace id.
    #[schemars(description = "")]
    CurrentWorkspaceId,
    /// Current tab id.
    #[schemars(description = "")]
    CurrentTabId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent view sort record.
#[schemars(description = "")]
pub struct AgentViewSort {
    /// Field.
    #[schemars(description = "")]
    pub field: AgentViewSortField,
    #[serde(default)]
    /// Order.
    #[schemars(description = "")]
    pub order: AgentViewSortOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
/// Agent view sort field record.
#[schemars(description = "")]
pub enum AgentViewSortField {
    /// Builtin.
    #[schemars(description = "")]
    Builtin(AgentViewBuiltinSortField),
    /// Token.
    #[schemars(description = "")]
    Token {
        /// Token.
        #[schemars(description = "")]
        token: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Agent view builtin sort field record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentViewBuiltinSortField {
    /// Workspace order.
    #[schemars(description = "")]
    WorkspaceOrder,
    /// Tab order.
    #[schemars(description = "")]
    TabOrder,
    /// Pane order.
    #[schemars(description = "")]
    PaneOrder,
    /// Attention.
    #[schemars(description = "")]
    Attention,
    /// Status.
    #[schemars(description = "")]
    Status,
    /// Agent.
    #[schemars(description = "")]
    Agent,
    /// Seen.
    #[schemars(description = "")]
    Seen,
    /// State change seq.
    #[schemars(description = "")]
    StateChangeSeq,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
/// Agent view sort order record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum AgentViewSortOrder {
    #[default]
    /// Asc.
    #[schemars(description = "")]
    Asc,
    /// Desc.
    #[schemars(description = "")]
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent start params record.
#[schemars(description = "")]
pub struct AgentStartParams {
    /// Name.
    #[schemars(description = "")]
    pub name: String,
    /// Kind.
    #[schemars(description = "")]
    pub kind: String,
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Args.
    #[schemars(description = "")]
    pub args: Vec<String>,
    /// Startup timeout in milliseconds. Values must be greater than 3000 and at most 300000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent prompt params record.
#[schemars(description = "")]
pub struct AgentPromptParams {
    /// Target.
    #[schemars(description = "")]
    pub target: String,
    /// Text.
    #[schemars(description = "")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Wait.
    #[schemars(description = "")]
    pub wait: Option<AgentPromptWaitOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent info record.
#[schemars(description = "")]
pub struct AgentInfo {
    /// Terminal id.
    #[schemars(description = "")]
    pub terminal_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Name.
    #[schemars(description = "")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent.
    #[schemars(description = "")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Title.
    #[schemars(description = "")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Terminal title.
    #[schemars(description = "")]
    pub terminal_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Terminal title stripped.
    #[schemars(description = "")]
    pub terminal_title_stripped: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Display agent.
    #[schemars(description = "")]
    pub display_agent: Option<String>,
    /// Agent status.
    #[schemars(description = "")]
    pub agent_status: AgentStatus,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Screen detection skipped.
    #[schemars(description = "")]
    pub screen_detection_skipped: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// State labels.
    #[schemars(description = "")]
    pub state_labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schemars(schema_with = "super::common::metadata_token_values_schema")]
    /// Tokens.
    #[schemars(description = "")]
    pub tokens: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent session.
    #[schemars(description = "")]
    pub agent_session: Option<AgentSessionInfo>,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Focused.
    #[schemars(description = "")]
    pub focused: crate::shared::Flag,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Launch pending.
    #[schemars(description = "")]
    pub launch_pending: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Interactive ready.
    #[schemars(description = "")]
    pub interactive_ready: bool,
    #[serde(default)]
    /// State change seq.
    #[schemars(description = "")]
    pub state_change_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Foreground cwd.
    #[schemars(description = "")]
    pub foreground_cwd: Option<String>,
    /// Revision.
    #[schemars(description = "")]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent session info record.
#[schemars(description = "")]
pub struct AgentSessionInfo {
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    /// Agent.
    #[schemars(description = "")]
    pub agent: String,
    /// Kind.
    #[schemars(description = "")]
    pub kind: crate::shared::AgentSessionRefKind,
    /// Value.
    #[schemars(description = "")]
    pub value: String,
}
