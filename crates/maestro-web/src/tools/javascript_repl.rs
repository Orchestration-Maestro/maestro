//! Browser-local default JavaScript execution registration.
use crate::browser::sandbox_value::BrowserSandboxValue;
use crate::components::sandboxed_iframe::BrowserSandboxHost;
thread_local! {
    /// Lazily initialized tool; each invocation obtains its own hidden sandbox.
    pub static JAVASCRIPT_REPL_TOOL: std::cell::LazyCell<maestro_app::browser::tools::javascript_repl::JavaScriptReplTool<BrowserSandboxValue>> = std::cell::LazyCell::new(|| maestro_app::create_javascript_repl_tool(std::sync::Arc::new(BrowserSandboxHost)));
}
