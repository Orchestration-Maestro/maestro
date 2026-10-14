//! Native tool selection with typed definitions and ordered presets.

pub use crate::edit::{
    EditOperations, EditToolDetails, EditToolInput, EditToolOptions, create_edit_tool,
    create_edit_tool_definition,
};
pub use crate::file_mutation_queue::with_file_mutation_queue;
pub use crate::output::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationOptions, TruncationResult, format_size,
    truncate_head, truncate_line, truncate_tail,
};
pub use crate::process::bash::{
    BashOperations, BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput,
    BashToolOptions, create_bash_tool, create_bash_tool_definition, create_local_bash_operations,
};
pub use crate::read::{
    ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, create_read_tool,
    create_read_tool_definition,
};
pub use crate::search::find::{
    FindOperations, FindToolDetails, FindToolInput, FindToolOptions, create_find_tool,
    create_find_tool_definition,
};
pub use crate::search::grep::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, create_grep_tool,
    create_grep_tool_definition,
};
pub use crate::search::ls::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
    create_ls_tool_definition,
};
pub use crate::write::{
    WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_definition,
};

use crate::{BashRenderState, EditRenderState, ReadContext, ToolDefinition, ToolsManager};
use indexmap::IndexMap;
use maestro_models::DiagnosticErrorInfo;
use std::{fmt, str::FromStr, sync::Arc};

/// An executable tool, including caller-defined names.
pub type Tool = maestro_agent::AgentTool;

/// A selected definition retaining its concrete arguments, details and render state.
pub enum ToolDef<C = ()> {
    /// The read definition.
    Read(ToolDefinition<C, ReadToolInput, ReadToolDetails>),
    /// The bash definition.
    Bash(ToolDefinition<C, BashToolInput, BashToolDetails, BashRenderState>),
    /// The edit definition.
    Edit(ToolDefinition<C, EditToolInput, EditToolDetails, EditRenderState>),
    /// The write definition.
    Write(ToolDefinition<C, WriteToolInput, ()>),
    /// The grep definition.
    Grep(ToolDefinition<C, GrepToolInput, GrepToolDetails>),
    /// The find definition.
    Find(ToolDefinition<C, FindToolInput, FindToolDetails>),
    /// The ls definition.
    Ls(ToolDefinition<C, LsToolInput, LsToolDetails>),
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
#[derive(Default)]
pub struct ToolsOptions {
    /// Configuration passed only to read.
    pub read: ReadToolOptions,
    /// Configuration passed only to bash.
    pub bash: BashToolOptions,
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

/// Select one built-in definition with its own options.
///
/// # Errors
/// Returns `Unknown tool name: <name>` for any other spelling.
pub fn create_tool_definition<C: ReadContext + Send + 'static>(
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
pub fn create_coding_tool_definitions<C: ReadContext + Send + 'static>(
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
pub fn create_read_only_tool_definitions<C: ReadContext + Send + 'static>(
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
pub fn create_all_tool_definitions<C: ReadContext + Send + 'static>(
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
