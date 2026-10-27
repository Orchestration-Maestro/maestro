//! Built-in constructors declared once on the shared import interface.

/// Declares native factories without creating a second adapter trait.
macro_rules! builtin_tool_imports {
    () => {
        /// Returns the native coding preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_coding_tools(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<Vec<crate::Tool>>;
        /// Returns the native coding preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_coding_tool_definitions(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<Vec<crate::ToolDef>>;
        /// Returns the native read-only preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_read_only_tools(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<Vec<crate::Tool>>;
        /// Returns the native read-only preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_read_only_tool_definitions(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<Vec<crate::ToolDef>>;
        /// Returns the native all preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_all_tools(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::Tool>>;
        /// Returns the native all preset without reordering.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_all_tool_definitions(
            &self,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::ToolDef>>;

        /// Selects an original native executable without normalizing its name.
        /// # Errors
        /// Returns native selection or admission failures.
        fn create_tool(
            &self,
            tool_name: &str,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Selects an original native definition without normalizing its name.
        /// # Errors
        /// Returns native selection or admission failures.
        fn create_tool_definition(
            &self,
            tool_name: &str,
            cwd: String,
            options: crate::ToolsOptions,
        ) -> ExtensionResult<crate::ToolDef>;
        /// Retains the original native local shell backend.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_local_bash_operations(
            &self,
            shell_path: Option<String>,
        ) -> ExtensionResult<crate::BashOperations>;
        /// Constructs the original context-free shell executable.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_bash_tool(
            &self,
            cwd: String,
            options: crate::BashToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the original shell definition with retained interception.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_bash_tool_definition(
            &self,
            cwd: String,
            options: crate::BashToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Runs a guest-local callback while retaining the native queue turn.
        fn with_file_mutation_queue(
            &self,
            file_path: String,
            operation: Box<dyn FnOnce() -> crate::ExtensionFuture<'static, ()>>,
        ) -> crate::ExtensionFuture<'static, ()>;
        /// Constructs the original context-free content search.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_grep_tool(
            &self,
            cwd: String,
            options: crate::GrepToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the original content search definition.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_grep_tool_definition(
            &self,
            cwd: String,
            options: crate::GrepToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Constructs the original context-free native directory lister.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_ls_tool(
            &self,
            cwd: String,
            options: crate::LsToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the original listing definition with retained observations.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_ls_tool_definition(
            &self,
            cwd: String,
            options: crate::LsToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Constructs an original context-free native finder.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_find_tool(
            &self,
            cwd: String,
            options: crate::FindToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the original find definition with retained effects.
        /// # Errors
        /// Returns construction or resource admission failures.
        fn create_find_tool_definition(
            &self,
            cwd: String,
            options: crate::FindToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Constructs the native edit executable with original argument preparation.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_edit_tool(
            &self,
            cwd: String,
            options: crate::EditToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the native edit definition with retained effects and renderers.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_edit_tool_definition(
            &self,
            cwd: String,
            options: crate::EditToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Constructs an original native executable without selecting an extension context.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_read_tool(
            &self,
            cwd: String,
            options: crate::ReadToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the original native read definition with owned replacement observations.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_read_tool_definition(
            &self,
            cwd: String,
            options: crate::ReadToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
        /// Constructs the native write executable over retained effects.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_write_tool(
            &self,
            cwd: String,
            options: crate::WriteToolOptions,
        ) -> ExtensionResult<crate::Tool>;
        /// Constructs the native write definition over retained effects.
        /// # Errors
        /// Returns native construction or resource admission failures.
        fn create_write_tool_definition(
            &self,
            cwd: String,
            options: crate::WriteToolOptions,
        ) -> ExtensionResult<crate::ToolDefinition>;
    };
}
