//! Existing browser tools owners exposed at the application root.

pub use crate::browser::tools::extract_document::{
    EXTRACT_DOCUMENT_TOOL, create_extract_document_tool,
};

/// Sandbox SDK provider contract.
pub use crate::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;

/// Sandbox SDK download, result and URL callback records.
pub use crate::browser::components::sandboxed_iframe::{
    SandboxFile, SandboxResult, SandboxUrlProvider,
};

/// JavaScript execution tool factory.
pub use crate::browser::tools::javascript_repl::create_javascript_repl_tool;

pub use crate::browser::tools::artifacts::{Artifact, ArtifactsPanel, ArtifactsParams};

pub use crate::browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ATTACHMENTS_RUNTIME_DESCRIPTION,
};
