//! Session picker header and session list presentation.
mod header;
pub use header::{
    SessionScope, SessionSelectorHeader, SessionSelectorHeaderOptions, StatusMessage,
};
pub(crate) mod list;
pub use list::{SessionList, SessionListOptions};

mod tree;

mod selector;
pub use selector::{
    SessionListProgress, SessionSelectorCallbacks, SessionSelectorComponent,
    SessionSelectorOptions, SessionsFuture, SessionsLoader,
};

mod delete_session_file;
pub use delete_session_file::{SessionFileOperations, TrashResult};
