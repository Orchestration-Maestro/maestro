//! Shared recording presentation for artifact reconstruction tests.
#![cfg(test)]
use maestro_app::BoxFuture;
use maestro_app::browser::tools::artifacts::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanelState, FileType,
    get_file_type,
};
use maestro_app::browser::tools::prompts::ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO;
use serde_json::{Value, json};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc as LogReader;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc as LogReader;
use std::sync::Mutex;

/// Recorded panel effects, provider observations and clock.
#[derive(Default)]
pub(crate) struct Recorder {
    /// Ordered presentation effects.
    events: Mutex<Vec<Value>>,
    /// Runtime providers observed by each HTML view operation.
    pub(crate) providers: Mutex<Vec<Value>>,
    /// Clock observations taken so far.
    pub(crate) clock: Mutex<i64>,
}
/// Summarize HTML runtime providers by kind, access and attachment identity.
fn observe(panel: &ArtifactsPanelState) -> Value {
    let providers = panel.get_html_artifact_runtime_providers::<Value>();
    Value::Array(
        providers
            .iter()
            .map(|provider| {
                let data = provider.get_data().unwrap();
                if let Some(attachments) = data.get("attachments") {
                    let ids = attachments
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|a| json!({"id": a["id"], "fileName": a["fileName"]}))
                        .collect::<Vec<_>>();
                    json!({"provider": "attachments", "attachments": ids})
                } else {
                    let description = provider.get_description().unwrap();
                    assert_eq!(description, ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO);
                    json!({"provider": "artifacts", "readWrite": false})
                }
            })
            .collect(),
    )
}
impl ArtifactPresentation for Recorder {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        let mut clock = self.clock.lock().unwrap();
        let date = chrono::DateTime::from_timestamp_millis(1_767_323_045_000 + *clock).unwrap();
        *clock += 1;
        date
    }
    fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
        panel: ArtifactsPanelState,
    ) {
        if get_file_type(filename) == FileType::Html {
            self.providers.lock().unwrap().push(observe(&panel));
        }
        self.events
            .lock()
            .unwrap()
            .push(json!(["element", filename, content]));
    }
    fn show_artifact(&self, filename: &str) {
        self.events.lock().unwrap().push(json!(["show", filename]));
    }
    fn remove_artifact_element(&self, filename: &str) {
        self.events
            .lock()
            .unwrap()
            .push(json!(["remove", filename]));
    }
    fn reload_all_html_artifacts(&self, panel: ArtifactsPanelState) {
        self.providers.lock().unwrap().push(observe(&panel));
        self.events.lock().unwrap().push(json!(["reload"]));
    }
    fn get_logs(&self, filename: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        if get_file_type(filename) == FileType::Html {
            return Ok(LogReader::new(|| "[log] view".into()));
        }
        Err(ArtifactLogError::Missing)
    }
    fn wait(&self, _: std::time::Duration) -> BoxFuture<()> {
        self.events.lock().unwrap().push(json!(["wait"]));
        Box::pin(std::future::ready(()))
    }
    fn request_update(&self) {
        self.events.lock().unwrap().push(json!(["update"]));
    }
    fn on_artifacts_change(&self) {
        self.events.lock().unwrap().push(json!(["change"]));
    }
}
impl Recorder {
    /// Take the effects recorded so far.
    pub(crate) fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}
