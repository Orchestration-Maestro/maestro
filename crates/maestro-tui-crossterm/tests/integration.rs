//! Terminal backend integration tests compiled as one program.
#![cfg(test)]

#[cfg(unix)]
mod lifecycle_support;
#[cfg(not(target_arch = "wasm32"))]
mod runtime_support;
#[cfg(unix)]
mod support;

mod example_theme_styles;
mod interactive_output_errors;
mod native_completion;
mod native_runtime;
mod terminal_lifecycle;
mod terminal_output;
mod windows_console;
mod windows_pipe;
