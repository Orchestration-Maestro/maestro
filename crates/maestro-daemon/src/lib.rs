//! Retained pane agent identity and presentation for the workspace daemon.

/// Validated session references and literal resume plans.
pub mod agent_resume;

/// Owned terminal input and supplied host geometry.
pub mod input;

/// Pane state and presentation operations.
pub mod terminal;

/// Host theme state and colour sequence emission.
pub mod terminal_theme;
