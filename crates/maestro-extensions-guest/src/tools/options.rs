//! Construction choices for the built-in tools.
use super::{
    BashToolOptions, EditToolOptions, FindToolOptions, GrepToolOptions, LsToolOptions,
    ReadToolOptions, WriteToolOptions,
};

/// Independent construction choices for each built-in tool.
#[derive(Default)]
pub struct ToolsOptions {
    /// Read observations and image policy.
    pub read: ReadToolOptions,
    /// Shell backend and interception.
    pub bash: BashToolOptions,
    /// Edit observations.
    pub edit: EditToolOptions,
    /// Write observations.
    pub write: WriteToolOptions,
    /// Content-search observations.
    pub grep: GrepToolOptions,
    /// File-search observations.
    pub find: FindToolOptions,
    /// Directory observations.
    pub ls: LsToolOptions,
}
