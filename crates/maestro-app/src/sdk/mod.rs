//! The embedding interface: creating a session with every default filled in, and the
//! records a program builds and replaces sessions with.
mod conversion;
mod creation;
mod options;
mod request;

pub use conversion::convert_to_llm_with_block_images;
pub use creation::create_agent_session;
pub use options::{CreateAgentSessionOptions, CreateAgentSessionResult, NoTools, SessionResources};

pub use request::{queue_mode, transform_context};
