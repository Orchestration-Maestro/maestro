//! Built-in constructors declared once on the shared import interface.

/// Declares native factories without creating a second adapter trait.
macro_rules! builtin_tool_imports {
    () => {
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
