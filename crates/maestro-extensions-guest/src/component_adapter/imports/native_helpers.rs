//! Native helper declarations on the shared import interface.

/// Declares helper calls without creating a second adapter trait.
macro_rules! native_helper_imports {
    () => {
        /// Formats all floating-point byte counts directly.
        fn native_format_size(&self, bytes: f64) -> ExtensionResult<String>;
        /// Reads metadata numbers without passing them through JSON.
        fn native_frontmatter(
            &self,
            content: &str,
        ) -> ExtensionResult<crate::bindings::maestro::extension::host::FrontmatterDocument>;
        /// Reads raw YAML through the same typed metadata transport.
        fn native_yaml(
            &self,
            content: &str,
        ) -> ExtensionResult<Vec<crate::bindings::maestro::extension::host::FrontmatterToken>>;
        /// Forwards ordinary helper operands synchronously.
        fn native_helper(&self, operation: &str, data: &str) -> ExtensionResult<String>;
    };
}
