//! Existing browser tools owners exposed at the application root.

pub use crate::browser::tools::extraction::{EXTRACT_DOCUMENT_TOOL, create_extract_document_tool};

/// Sandbox SDK provider contract.
pub use crate::browser::components::sandbox::providers::SandboxRuntimeProvider;

/// Sandbox SDK download, result and URL callback records.
pub use crate::browser::components::sandboxed_iframe::{
    SandboxFile, SandboxResult, SandboxUrlProvider,
};

/// JavaScript execution tool factory.
pub use crate::browser::tools::javascript_repl::create_javascript_repl_tool;

pub use crate::browser::tools::documents::{Artifact, ArtifactsPanel, ArtifactsParams};

pub use crate::browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ATTACHMENTS_RUNTIME_DESCRIPTION,
};
