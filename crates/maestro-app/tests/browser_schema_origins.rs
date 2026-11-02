//! Browser tool declarations and the schema nodes their authors declared.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::sync::Arc;

use maestro_app::SandboxResult;
use maestro_app::browser::components::sandboxed_iframe::{
    SandboxHost, SandboxIframe, SandboxUrlProvider,
};
use maestro_app::browser::tools::artifacts::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanel, ArtifactsPanelState,
};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, ParameterSchema, SchemaKind};
use serde::Deserialize;
use serde_json::Value;

/// One recorded producer case of the shared corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Case label, matching the browser tool name.
    #[serde(rename = "case")]
    label: String,
    /// Recorded group; only producer rows are read here.
    group: String,
    /// Supplied schema value.
    schema: Value,
    /// Declared node kinds by JSON pointer.
    kinds: BTreeMap<String, SchemaKind>,
    /// Recorded original arguments.
    arguments: serde_json::Map<String, Value>,
    /// Recorded prepared arguments or diagnostic.
    expected: Value,
}

/// The recorded producer case for one tool name.
fn recorded(name: &str) -> Case {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../maestro-models/tests/fixtures/tool_argument_conversion.json"
    ))
    .expect("recorded conversion corpus");
    let mut selected: Vec<Case> = cases
        .into_iter()
        .filter(|case| case.group == "producer" && case.label == name)
        .collect();
    assert_eq!(selected.len(), 1, "one recorded producer case for {name}");
    let case = selected.pop().expect("recorded case");
    assert!(!case.schema.is_null());
    assert!(!case.kinds.is_empty());
    assert!(!case.arguments.is_empty());
    case
}

/// Assert one declaration keeps the recorded JSON, node kinds and prepared outcome.
#[track_caller]
fn assert_declaration(name: &str, parameters: &ParameterSchema) {
    let case = recorded(name);
    assert_eq!(parameters.as_json(), &case.schema, "{name} schema");
    assert_eq!(
        parameters.conversions().len(),
        case.kinds.len(),
        "{name} kinds"
    );
    for (pointer, kind) in &case.kinds {
        assert_eq!(
            parameters.conversions().get(pointer),
            Some(kind),
            "{name} kind at {pointer:?}"
        );
    }
    let tool = maestro_models::Tool {
        name: name.to_owned(),
        description: String::new(),
        parameters: parameters.clone(),
    };
    let call = maestro_models::ToolCall {
        id: "call".to_owned(),
        name: name.to_owned(),
        arguments: Value::Object(case.arguments.clone()),
        thought_signature: None,
    };
    let outcome = match maestro_models::validate_tool_arguments(&tool, &call) {
        Ok(arguments) => serde_json::json!({"Ok": arguments}),
        Err(error) => serde_json::json!({"Err": error.message}),
    };
    assert_eq!(outcome, case.expected, "{name} prepared outcome");
}

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
        _: maestro_app::browser::components::sandboxed_iframe::SandboxExecution,
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
    let document = maestro_app::create_extract_document_tool(None);
    let declared = document
        .tool
        .read()
        .expect("declared tool")
        .definition
        .parameters
        .clone();
    assert_declaration("extract_document", &declared);

    let repl = maestro_app::create_javascript_repl_tool(Arc::new(QuietHost));
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

    let command = recorded("artifacts");
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
