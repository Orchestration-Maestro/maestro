//! Browser tool declarations and the schema nodes their authors declared.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use maestro_models::test_support::{assert_declaration, producer_case};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, SchemaKind};
use maestro_web::SandboxResult;
use maestro_web::components::browser::sandboxed_iframe::{
    SandboxHost, SandboxIframe, SandboxUrlProvider,
};
use maestro_web::tools::browser::documents::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanel, ArtifactsPanelState,
};

/// A sandbox host that answers every execution without output.
struct QuietHost;

impl SandboxHost for QuietHost {
    fn create_hidden(
        &self,
        _: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe>, DiagnosticErrorInfo> {
        Ok(Box::new(QuietHost))
    }
}

impl SandboxIframe for QuietHost {
    fn execute(
        &self,
        _: maestro_web::components::browser::sandboxed_iframe::SandboxExecution,
    ) -> BoxFuture<Result<SandboxResult, DiagnosticErrorInfo>> {
        Box::pin(async {
            Ok(SandboxResult {
                success: true,
                console: Vec::new(),
                files: None,
                error: None,
                return_value: None,
            })
        })
    }

    fn remove(&self) {}
}

/// A presentation that keeps no view and reports no log.
struct NoViews;

impl ArtifactPresentation for NoViews {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(1_767_323_045_000).expect("fixed instant")
    }

    fn get_or_create_artifact_element(&self, _: &str, _: &str, _: ArtifactsPanelState) {}

    fn show_artifact(&self, _: &str) {}

    fn remove_artifact_element(&self, _: &str) {}

    fn reload_all_html_artifacts(&self, _: ArtifactsPanelState) {}

    fn get_logs(&self, _: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        Err(ArtifactLogError::Missing)
    }

    fn wait(&self, _: std::time::Duration) -> BoxFuture<()> {
        Box::pin(async {})
    }

    fn request_update(&self) {}
}

#[test]
fn browser_tool_schemas_keep_authored_and_plain_nodes() {
    let document = maestro_web::create_extract_document_tool(None);
    let declared = document
        .tool
        .read()
        .expect("declared tool")
        .definition
        .parameters
        .clone();
    assert_declaration("extract_document", &declared);

    let repl = maestro_web::create_javascript_repl_tool(Arc::new(QuietHost));
    let declared = repl
        .tool
        .read()
        .expect("declared tool")
        .definition
        .parameters
        .clone();
    assert_declaration("javascript_repl", &declared);

    let panel = ArtifactsPanel::new(Arc::new(NoViews));
    let artifacts = panel.tool();
    assert_declaration("artifacts", &artifacts.definition.parameters);

    let command = producer_case("artifacts");
    assert!(
        !command.kinds.contains_key("/properties/command"),
        "the artifact command enum stays untagged"
    );
    assert_eq!(
        command.kinds.get("/properties/filename"),
        Some(&SchemaKind::String),
        "the neighboring text members stay authored"
    );
}
