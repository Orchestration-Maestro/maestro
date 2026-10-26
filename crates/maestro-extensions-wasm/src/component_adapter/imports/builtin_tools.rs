//! Built-in constructors declared once on the shared import interface.

/// Declares native factories without creating a second adapter trait.
macro_rules! builtin_tool_imports {
    () => {
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
