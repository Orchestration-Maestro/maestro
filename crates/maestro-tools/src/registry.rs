//! Native tool selection with typed definitions and ordered presets.

pub use crate::editing::{
    EditOperations, EditToolDetails, EditToolInput, EditToolOptions, create_edit_tool,
    create_edit_tool_definition,
};
pub use crate::mutations::with_file_mutation_queue;
pub use crate::output::limits::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationOptions, TruncationResult, format_size,
    truncate_head, truncate_line, truncate_tail,
};
pub use crate::process::command::{
    BashOperations, BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput,
    BashToolOptions, create_bash_tool, create_bash_tool_definition, create_local_bash_operations,
};
pub use crate::reading::{
    ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, create_read_tool,
    create_read_tool_definition,
};
pub use crate::search::directory::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
    create_ls_tool_definition,
};
pub use crate::search::discovery::{
    FindOperations, FindToolDetails, FindToolInput, FindToolOptions, create_find_tool,
    create_find_tool_definition,
};
pub use crate::search::matching::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, create_grep_tool,
    create_grep_tool_definition,
};
pub use crate::writing::{
    WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_definition,
};

use crate::process::command::{BashExecution, BashToolOptionsRecord};
use crate::types::{ToolDefinitionRecord, ToolExecution};
use crate::{BashRenderState, EditRenderState, ReadContext, ToolsManager};
use indexmap::IndexMap;
use maestro_models::DiagnosticErrorInfo;
use maestro_models::{Local, Threaded};
use serde_json::Value;
use std::{fmt, str::FromStr, sync::Arc};

/// An executable tool, including caller-defined names.
pub type Tool = maestro_agent::AgentTool;

/// Default thread-safe selected definition.
pub type ToolDef<C = ()> = ToolDefRecord<C, Threaded>;

/// A selected definition retaining its concrete arguments, details and render state.
pub enum ToolDefRecord<C = (), E: ToolExecution = Threaded> {
    /// The read definition.
    Read(ToolDefinitionRecord<C, ReadToolInput, ReadToolDetails, Value, E>),
    /// The bash definition.
    Bash(ToolDefinitionRecord<C, BashToolInput, BashToolDetails, BashRenderState, E>),
    /// The edit definition.
    Edit(ToolDefinitionRecord<C, EditToolInput, EditToolDetails, EditRenderState, E>),
    /// The write definition.
    Write(ToolDefinitionRecord<C, WriteToolInput, (), Value, E>),
    /// The grep definition.
    Grep(ToolDefinitionRecord<C, GrepToolInput, GrepToolDetails, Value, E>),
    /// The find definition.
    Find(ToolDefinitionRecord<C, FindToolInput, FindToolDetails, Value, E>),
    /// The ls definition.
    Ls(ToolDefinitionRecord<C, LsToolInput, LsToolDetails, Value, E>),
}

impl<C, E: ToolExecution> ToolDefRecord<C, E> {
    /// Project factory renderers without decoding their execution records.
    ///
    /// # Errors
    /// Rejects a substituted typed result slot; use the custom binding constructor instead.
    pub fn renderer_binding(&self) -> Result<crate::ToolRendererBinding, DiagnosticErrorInfo> {
        match self {
            Self::Read(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Bash(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Edit(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Write(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Grep(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Find(tool) => crate::ToolRendererBinding::from_raw(tool),
            Self::Ls(tool) => crate::ToolRendererBinding::from_raw(tool),
        }
    }
}

/// The built-in tool identities; custom executable names remain unrestricted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolName {
    /// The read tool.
    Read,
    /// The bash tool.
    Bash,
    /// The edit tool.
    Edit,
    /// The write tool.
    Write,
    /// The grep tool.
    Grep,
    /// The find tool.
    Find,
    /// The ls tool.
    Ls,
}

impl FromStr for ToolName {
    type Err = DiagnosticErrorInfo;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "read" => Ok(Self::Read),
            "bash" => Ok(Self::Bash),
            "edit" => Ok(Self::Edit),
            "write" => Ok(Self::Write),
            "grep" => Ok(Self::Grep),
            "find" => Ok(Self::Find),
            "ls" => Ok(Self::Ls),
            _ => Err(DiagnosticErrorInfo::error(format!(
                "Unknown tool name: {name}"
            ))),
        }
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Bash => "bash",
            Self::Edit => "edit",
            Self::Write => "write",
            Self::Grep => "grep",
            Self::Find => "find",
            Self::Ls => "ls",
        })
    }
}

/// All built-in identities in factory insertion order.
pub const ALL_TOOL_NAMES: [ToolName; 7] = [
    ToolName::Read,
    ToolName::Bash,
    ToolName::Edit,
    ToolName::Write,
    ToolName::Grep,
    ToolName::Find,
    ToolName::Ls,
];

/// Individual tool configuration; each member keeps its own default.
pub struct ToolsOptionsRecord<E: BashExecution = Threaded> {
    /// Configuration passed only to read.
    pub read: ReadToolOptions,
    /// Configuration passed only to bash.
    pub bash: BashToolOptionsRecord<E>,
    /// Configuration passed only to write.
    pub write: WriteToolOptions,
    /// Configuration passed only to edit.
    pub edit: EditToolOptions,
    /// Configuration passed only to grep.
    pub grep: GrepToolOptions,
    /// Configuration passed only to find.
    pub find: FindToolOptions,
    /// Configuration passed only to ls.
    pub ls: LsToolOptions,
}

impl<E: BashExecution> Default for ToolsOptionsRecord<E> {
    fn default() -> Self {
        Self {
            read: ReadToolOptions::default(),
            bash: BashToolOptionsRecord::default(),
            write: WriteToolOptions::default(),
            edit: EditToolOptions::default(),
            grep: GrepToolOptions::default(),
            find: FindToolOptions::default(),
            ls: LsToolOptions::default(),
        }
    }
}
/// Default thread-safe built-in configuration.
pub type ToolsOptions = ToolsOptionsRecord<Threaded>;

/// One selector and preset implementation for both concrete factory families.
macro_rules! factories { ([$($bound:tt)*]) => {
/// Select one built-in definition with its own options.
///
/// # Errors
/// Returns `Unknown tool name: <name>` for any other spelling.
pub fn create_tool_definition<C: ReadContext + 'static $($bound)*>(
    tool_name: &str,
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Result<ToolDef<C>, DiagnosticErrorInfo> {
    Ok(match tool_name.parse::<ToolName>()? {
        ToolName::Read => ToolDef::Read(create_read_tool_definition(cwd, options.read)),
        ToolName::Bash => ToolDef::Bash(create_bash_tool_definition(cwd, bin_dir, options.bash)),
        ToolName::Edit => ToolDef::Edit(create_edit_tool_definition(cwd, options.edit)),
        ToolName::Write => ToolDef::Write(create_write_tool_definition(cwd, options.write)),
        ToolName::Grep => ToolDef::Grep(create_grep_tool_definition(
            cwd,
            options.grep,
            tools_manager,
        )),
        ToolName::Find => ToolDef::Find(create_find_tool_definition(
            cwd,
            options.find,
            tools_manager,
        )),
        ToolName::Ls => ToolDef::Ls(create_ls_tool_definition(cwd, options.ls)),
    })
}

/// Construct read, bash, edit, write in that order.
#[must_use]
pub fn create_coding_tool_definitions<C: ReadContext + 'static $($bound)*>(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Vec<ToolDef<C>> {
    drop(tools_manager);
    vec![
        ToolDef::Read(create_read_tool_definition(cwd.clone(), options.read)),
        ToolDef::Bash(create_bash_tool_definition(
            cwd.clone(),
            bin_dir,
            options.bash,
        )),
        ToolDef::Edit(create_edit_tool_definition(cwd.clone(), options.edit)),
        ToolDef::Write(create_write_tool_definition(cwd, options.write)),
    ]
}

/// Construct read, grep, find, ls in that order.
#[must_use]
pub fn create_read_only_tool_definitions<C: ReadContext + 'static $($bound)*>(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Vec<ToolDef<C>> {
    drop(bin_dir);
    vec![
        ToolDef::Read(create_read_tool_definition(cwd.clone(), options.read)),
        ToolDef::Grep(create_grep_tool_definition(
            cwd.clone(),
            options.grep,
            Arc::clone(&tools_manager),
        )),
        ToolDef::Find(create_find_tool_definition(
            cwd.clone(),
            options.find,
            tools_manager,
        )),
        ToolDef::Ls(create_ls_tool_definition(cwd, options.ls)),
    ]
}

/// Construct read, bash, edit, write, grep, find, ls in that order.
#[must_use]
pub fn create_all_tool_definitions<C: ReadContext + 'static $($bound)*>(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> IndexMap<ToolName, ToolDef<C>> {
    IndexMap::from([
        (
            ToolName::Read,
            ToolDef::Read(create_read_tool_definition(cwd.clone(), options.read)),
        ),
        (
            ToolName::Bash,
            ToolDef::Bash(create_bash_tool_definition(
                cwd.clone(),
                bin_dir,
                options.bash,
            )),
        ),
        (
            ToolName::Edit,
            ToolDef::Edit(create_edit_tool_definition(cwd.clone(), options.edit)),
        ),
        (
            ToolName::Write,
            ToolDef::Write(create_write_tool_definition(cwd.clone(), options.write)),
        ),
        (
            ToolName::Grep,
            ToolDef::Grep(create_grep_tool_definition(
                cwd.clone(),
                options.grep,
                Arc::clone(&tools_manager),
            )),
        ),
        (
            ToolName::Find,
            ToolDef::Find(create_find_tool_definition(
                cwd.clone(),
                options.find,
                tools_manager,
            )),
        ),
        (
            ToolName::Ls,
            ToolDef::Ls(create_ls_tool_definition(cwd, options.ls)),
        ),
    ])
}

/// Select one built-in executable tool with its own options.
///
/// # Errors
/// Returns `Unknown tool name: <name>` for any other spelling.
pub fn create_tool(
    tool_name: &str,
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Result<Tool, DiagnosticErrorInfo> {
    Ok(match tool_name.parse::<ToolName>()? {
        ToolName::Read => create_read_tool(cwd, options.read),
        ToolName::Bash => create_bash_tool(cwd, bin_dir, options.bash),
        ToolName::Edit => create_edit_tool(cwd, options.edit),
        ToolName::Write => create_write_tool(cwd, options.write),
        ToolName::Grep => create_grep_tool(cwd, options.grep, tools_manager),
        ToolName::Find => create_find_tool(cwd, options.find, tools_manager),
        ToolName::Ls => create_ls_tool(cwd, options.ls),
    })
}

/// Construct read, bash, edit, write in that order.
#[must_use]
pub fn create_coding_tools(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Vec<Tool> {
    drop(tools_manager);
    vec![
        create_read_tool(cwd.clone(), options.read),
        create_bash_tool(cwd.clone(), bin_dir, options.bash),
        create_edit_tool(cwd.clone(), options.edit),
        create_write_tool(cwd, options.write),
    ]
}

/// Construct read, grep, find, ls in that order.
#[must_use]
pub fn create_read_only_tools(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> Vec<Tool> {
    drop(bin_dir);
    vec![
        create_read_tool(cwd.clone(), options.read),
        create_grep_tool(cwd.clone(), options.grep, Arc::clone(&tools_manager)),
        create_find_tool(cwd.clone(), options.find, tools_manager),
        create_ls_tool(cwd, options.ls),
    ]
}

/// Construct read, bash, edit, write, grep, find, ls in that order.
#[must_use]
pub fn create_all_tools(
    cwd: String,
    options: ToolsOptions,
    bin_dir: String,
    tools_manager: Arc<ToolsManager>,
) -> IndexMap<ToolName, Tool> {
    IndexMap::from([
        (ToolName::Read, create_read_tool(cwd.clone(), options.read)),
        (
            ToolName::Bash,
            create_bash_tool(cwd.clone(), bin_dir, options.bash),
        ),
        (ToolName::Edit, create_edit_tool(cwd.clone(), options.edit)),
        (
            ToolName::Write,
            create_write_tool(cwd.clone(), options.write),
        ),
        (
            ToolName::Grep,
            create_grep_tool(cwd.clone(), options.grep, Arc::clone(&tools_manager)),
        ),
        (
            ToolName::Find,
            create_find_tool(cwd.clone(), options.find, tools_manager),
        ),
        (ToolName::Ls, create_ls_tool(cwd, options.ls)),
    ])
}

}; }
factories!([+ Send]);
/// Caller-local selectors and ordered built-in presets.
pub mod local {
    use super::{
        Arc, DiagnosticErrorInfo, IndexMap, Local, ReadContext, ToolDefRecord, ToolName,
        ToolsManager, ToolsOptionsRecord,
    };
    pub use crate::{
        editing::local::*,
        process::command::local::*,
        reading::local::*,
        search::{directory::local::*, discovery::local::*, matching::local::*},
        writing::local::*,
    };
    /// Caller-local selected definition with unchanged typed variants.
    pub type ToolDef<C = ()> = ToolDefRecord<C, Local>;
    /// Caller-local built-in configuration.
    pub type ToolsOptions = ToolsOptionsRecord<Local>;
    /// Caller-local executable agent tool.
    pub type Tool = maestro_agent::AgentToolRecord<Local>;
    factories!([]);
}
