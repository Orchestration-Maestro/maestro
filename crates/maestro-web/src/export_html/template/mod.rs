//! Offline session document preparation and header widgets.
mod document;
pub use document::{SessionDocument, load_document};
mod details;
mod header;
pub use header::{HeaderAction, HeaderState, HeaderView, format_tokens, render_header};
mod stats;
#[cfg(target_arch = "wasm32")]
pub use header::localize_header_date;
pub use stats::{Stats, compute_stats};
