//! Owned session document preparation.
mod document;
pub use document::{SessionDocument, load_document};
mod stats;
pub use stats::{Stats, compute_stats};

mod header;

#[cfg(target_arch = "wasm32")]
pub use header::localize_header_date;
pub use header::{HeaderAction, HeaderState, HeaderView, render_header};

mod download;
pub use download::download_session_json;

mod copy_link;
pub use copy_link::{CopyLinkState, CopyLinkView, render_copy_link_button};

mod markdown;
pub use markdown::safe_marked_parse;
mod output;
pub use output::format_expandable_output;

#[cfg(test)]
mod tests;
mod values;

#[cfg(target_arch = "wasm32")]
pub use output::bind_expandable_output;
