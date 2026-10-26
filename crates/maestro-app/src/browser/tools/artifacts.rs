//! Retained artifact commands with caller-supplied presentation operations.
use super::super::components::sandbox::{
    artifacts_runtime_provider::{AgentLike, ArtifactsRuntimeProvider},
    attachments_runtime_provider::AttachmentsRuntimeProvider,
    sandbox_runtime_provider::{RuntimeProviderValue, SandboxRuntimeProvider},
};
use super::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ATTACHMENTS_RUNTIME_DESCRIPTION,
    artifacts_tool_description,
};
use crate::utils::lock::{read, write};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use maestro_agent::{AgentTool, AgentToolResult, LocalAgentTool};
use maestro_models::{
    BoxFuture, Execution, SharedHandle, StringEnumOptions, Threaded, Tool, present, string_enum,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Mutable file with independently observed creation and update dates.
pub struct Artifact {
    /// Authored filename.
    pub filename: String,
    /// Literal file content.
    pub content: String,
    /// Creation clock observation.
    pub created_at: DateTime<Utc>,
    /// Most recent edit clock observation.
    pub updated_at: DateTime<Utc>,
}
/// Operations admitted by the artifact tool.
#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub enum ArtifactCommand {
    /// Create a new file.
    Create,
    /// Replace its first matching substring.
    Update,
    /// Replace all content.
    Rewrite,
    /// Read content.
    Get,
    /// Remove a file.
    Delete,
    /// Read current HTML console output.
    Logs,
}
impl TryFrom<String> for ArtifactCommand {
    type Error = &'static str;
    /// Admit only the six string-valued command tags.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "create" => Ok(Self::Create),
            "update" => Ok(Self::Update),
            "rewrite" => Ok(Self::Rewrite),
            "get" => Ok(Self::Get),
            "delete" => Ok(Self::Delete),
            "logs" => Ok(Self::Logs),
            _ => Err("unknown artifact command"),
        }
    }
}
/// Artifact operation arguments.
#[derive(Deserialize)]
pub struct ArtifactsParams {
    /// Selected operation.
    pub command: ArtifactCommand,
    /// Authored filename.
    pub filename: String,
    /// New complete content.
    #[serde(default, deserialize_with = "present")]
    pub content: Option<String>,
    /// Literal substring to replace.
    #[serde(default, deserialize_with = "present")]
    pub old_str: Option<String>,
    /// Literal replacement, including an empty string.
    #[serde(default, deserialize_with = "present")]
    pub new_str: Option<String>,
}
/// Object-only tool ingress.
#[derive(Deserialize)]
struct Parameters(#[serde(deserialize_with = "maestro_json::object::record")] ArtifactsParams);
/// Retain ordered mutable maps and their independently mutable records.
pub type Artifacts = Arc<RwLock<IndexMap<String, Arc<RwLock<Artifact>>>>>;
/// View-specific log failures.
pub enum ArtifactLogError {
    /// No corresponding view exists.
    Missing,
    /// The view is not HTML.
    NotHtml,
}
/// Retained native log operation.
#[cfg(not(target_arch = "wasm32"))]
pub type ArtifactLogReader = Arc<dyn Fn() -> String + Send + Sync>;
/// Retained browser-local log operation.
#[cfg(target_arch = "wasm32")]
pub type ArtifactLogReader = std::rc::Rc<dyn Fn() -> String>;
/// Replaceable artifact view and clock operations.
pub trait ArtifactPresentation {
    /// Observe the current date.
    fn now(&self) -> DateTime<Utc>;
    /// Upsert a view; an HTML view takes its runtime providers from the live panel state.
    fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
        panel: ArtifactsPanelState,
    );
    /// Select a view.
    fn show_artifact(&self, filename: &str);
    /// Remove its retained view.
    fn remove_artifact_element(&self, filename: &str);
    /// Refresh mounted HTML views with runtime providers from the live panel state.
    fn reload_all_html_artifacts(&self, panel: ArtifactsPanelState);
    /// Capture the current HTML view's log reader.
    ///
    /// # Errors
    /// Distinguishes absent and non-HTML views.
    fn get_logs(&self, filename: &str) -> Result<ArtifactLogReader, ArtifactLogError>;
    /// Wait for the supplied duration.
    fn wait(&self, duration: Duration) -> BoxFuture<()>;
    /// Request a presentation update.
    fn request_update(&self);
    /// Notify a file change when configured.
    fn on_artifacts_change(&self) {}
    /// Request the host to open the panel when configured.
    fn on_open(&self) {}
}
/// Native shared presentation operations.
#[cfg(not(target_arch = "wasm32"))]
pub type SharedArtifactPresentation = Arc<dyn ArtifactPresentation + Send + Sync>;
/// Browser-local shared presentation operations.
#[cfg(target_arch = "wasm32")]
pub type SharedArtifactPresentation = Arc<dyn ArtifactPresentation>;
/// Current replaceable map slot, selected filename and conversation.
struct State {
    /// Current ordered map owner.
    artifacts: Artifacts,
    /// Selected filename, if any.
    active: Option<String>,
    /// Conversation whose attachments HTML artifacts can read, while its owner keeps it alive.
    agent: Option<<Threaded as Execution>::Weak<dyn AgentLike>>,
}
/// Shared panel state, independent of its presentation operations.
#[derive(Clone)]
pub struct ArtifactsPanelState(Arc<RwLock<State>>);
impl ArtifactsPanelState {
    /// Return the current map handle; create and delete replace its container.
    #[must_use]
    pub fn artifacts(&self) -> Artifacts {
        Arc::clone(&read(&self.0).artifacts)
    }
    /// Runtime providers for HTML artifacts: the conversation's attachments, when it
    /// has any, then read-only access to this panel's artifacts.
    #[must_use]
    pub fn get_html_artifact_runtime_providers<V: RuntimeProviderValue>(
        &self,
    ) -> Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>> {
        let agent = read(&self.0).agent.as_ref().and_then(Threaded::upgrade);
        let attachments = agent.map(|agent| agent.attachments()).unwrap_or_default();
        let mut providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>> = Vec::new();
        if !attachments.is_empty() {
            providers.push(SharedHandle::new(AttachmentsRuntimeProvider::new(
                Arc::new(RwLock::new(attachments)),
            )));
        }
        providers.push(SharedHandle::new(ArtifactsRuntimeProvider::read_only(
            self.clone(),
        )));
        providers
    }
}
/// Retained command policy, independent of a concrete browser panel.
#[derive(Clone)]
pub struct ArtifactsPanel<P = SharedArtifactPresentation> {
    /// Shared current state slot.
    pub(crate) state: ArtifactsPanelState,
    /// Owned view and clock operations.
    pub(crate) presentation: P,
}
/// Reconstruction-only notification and wait controls.
#[derive(Default)]
pub(crate) struct Options {
    /// Suppress mutation notifications.
    silent: bool,
    /// Suppress the HTML log delay.
    skip_wait: bool,
}
/// Local presentation pointer from the shared execution representation.
pub type LocalArtifactPresentation =
    <maestro_models::Local as maestro_models::Execution>::Shared<dyn ArtifactPresentation>;
/// Retained caller accepting local presentation captures on every target.
pub type LocalArtifactsPanel = ArtifactsPanel<LocalArtifactPresentation>;
/// One constructor and executable template for both supported pointer families.
macro_rules! caller {
    ($presentation:ty, $new:ident, $tool:ident, $module:ident, $pointer:ident) => {
        impl ArtifactsPanel<$presentation> {
            /// Retain the supplied presentation operations with an empty store.
            #[must_use]
            pub fn $new(presentation: $presentation) -> Self {
                Self {
                    state: ArtifactsPanelState(Arc::new(RwLock::new(State {
                        artifacts: Arc::new(RwLock::new(IndexMap::new())),
                        active: None,
                        agent: None,
                    }))),
                    presentation,
                }
            }
            /// Build an executable declaration retaining this same command state.
            #[must_use]
            pub fn tool(&self) -> $tool {
                let panel = self.clone();
                $tool::typed(
                    definition(),
                    "Artifacts".into(),
                    std::$module::$pointer::new(move |_, Parameters(params), _, _| {
                        let panel = panel.clone();
                        Box::pin(async move {
                            let text = panel
                                .execute(&params, Options::default())
                                .await
                                .unwrap_or_else(|error| error);
                            Ok(AgentToolResult::<()>::text_result(text, None))
                        })
                    }),
                )
            }
        }
    };
}
#[cfg(not(target_arch = "wasm32"))]
caller!(SharedArtifactPresentation, new, AgentTool, sync, Arc);
#[cfg(target_arch = "wasm32")]
caller!(SharedArtifactPresentation, new, AgentTool, rc, Rc);
caller!(
    std::rc::Rc<dyn ArtifactPresentation>,
    new_local,
    LocalAgentTool,
    rc,
    Rc
);
impl<P> ArtifactsPanel<P>
where
    P: Clone + std::ops::Deref,
    P::Target: ArtifactPresentation,
{
    /// Return the current map handle; create and delete replace its container.
    #[must_use]
    pub fn artifacts(&self) -> Artifacts {
        self.state.artifacts()
    }
    /// Observe the shared selected filename, including unknown or empty names.
    #[must_use]
    pub fn active_filename(&self) -> Option<String> {
        read(&self.state.0).active.clone()
    }
    /// Retain the conversation whose attachments HTML artifacts can read without owning it.
    pub fn set_agent(&self, agent: Option<&SharedHandle<dyn AgentLike>>) {
        write(&self.state.0).agent = agent.map(Threaded::downgrade);
    }
    /// Select an existing file and request that the host open the panel.
    pub fn open_artifact(&self, filename: &str) {
        let exists = read(&self.artifacts()).contains_key(filename);
        if exists {
            self.show_artifact(filename);
            self.presentation.on_open();
        }
    }
    /// Select a filename without holding the state guard through host operations.
    pub fn show_artifact(&self, filename: &str) {
        self.presentation.show_artifact(filename);
        write(&self.state.0).active = Some(filename.into());
        self.presentation.request_update();
    }
    /// Dispatch one typed command; a failure keeps its returned error text.
    pub(crate) async fn execute(
        &self,
        params: &ArtifactsParams,
        options: Options,
    ) -> Result<String, String> {
        match params.command {
            ArtifactCommand::Create => self.create(params, options).await,
            ArtifactCommand::Update | ArtifactCommand::Rewrite => self.edit(params, options).await,
            ArtifactCommand::Get => {
                let record = self.find(&params.filename)?;
                Ok(read(&record).content.clone())
            }
            ArtifactCommand::Delete => self.delete(&params.filename),
            ArtifactCommand::Logs => self.logs(&params.filename),
        }
    }
    /// Format the absent-file response from the current ordered keys.
    fn missing(&self, filename: &str) -> String {
        let files = read(&self.artifacts()).keys().cloned().collect::<Vec<_>>();
        if files.is_empty() {
            format!("Error: File {filename} not found. No files have been created yet.")
        } else {
            format!(
                "Error: File {filename} not found. Available files: {}",
                files.join(", ")
            )
        }
    }
    /// Capture the current record without retaining the map guard.
    fn find(&self, filename: &str) -> Result<Arc<RwLock<Artifact>>, String> {
        let record = read(&self.artifacts()).get(filename).cloned();
        record.ok_or_else(|| self.missing(filename))
    }
    /// Replace the current map container after mutating its surviving aliases.
    fn replace_map(&self, artifacts: &Artifacts) {
        let copy = read(artifacts).clone();
        write(&self.state.0).artifacts = Arc::new(RwLock::new(copy));
    }
    /// Create a dated record before issuing presentation effects.
    async fn create(&self, params: &ArtifactsParams, options: Options) -> Result<String, String> {
        let artifacts = self.artifacts();
        let exists = read(&artifacts).contains_key(&params.filename);
        let content = created_content(params, exists)?;
        self.insert(&artifacts, &params.filename, content);
        self.replace_map(&artifacts);
        Ok(self.created(&params.filename, content, options).await)
    }
    /// Insert a new record dated by two clock observations.
    fn insert(&self, artifacts: &Artifacts, filename: &str, content: &str) {
        let record = Artifact {
            filename: filename.into(),
            content: content.into(),
            created_at: self.presentation.now(),
            updated_at: self.presentation.now(),
        };
        write(artifacts).insert(filename.into(), Arc::new(RwLock::new(record)));
    }
    /// Show a newly inserted file, notify unless silent, then refresh its dependents.
    async fn created(&self, filename: &str, content: &str, options: Options) -> String {
        self.presentation
            .get_or_create_artifact_element(filename, content, self.state.clone());
        if !options.silent {
            self.show_artifact(filename);
            self.presentation.on_artifacts_change();
            self.presentation.request_update();
        }
        self.finish(
            filename,
            format!("Created file {filename}"),
            options.skip_wait,
        )
        .await
    }
    /// Edit a retained record and publish its content before notifications.
    async fn edit(&self, params: &ArtifactsParams, options: Options) -> Result<String, String> {
        let record = self.find(&params.filename)?;
        let content = edited_content(params, &read(&record).content)?;
        let now = self.presentation.now();
        {
            let mut record = write(&record);
            record.content.clone_from(&content);
            record.updated_at = now;
        }
        write(&self.artifacts()).insert(params.filename.clone(), record);
        self.presentation.get_or_create_artifact_element(
            &params.filename,
            &content,
            self.state.clone(),
        );
        let update = matches!(params.command, ArtifactCommand::Update);
        if !options.silent {
            self.presentation.on_artifacts_change();
            if update {
                self.presentation.request_update();
            }
        }
        self.show_artifact(&params.filename);
        let result = if update {
            format!("Updated file {}", params.filename)
        } else {
            String::new()
        };
        Ok(self
            .finish(&params.filename, result, options.skip_wait)
            .await)
    }
    /// Refresh dependencies, then capture and await the selected HTML log reader.
    async fn finish(&self, filename: &str, mut result: String, skip_wait: bool) -> String {
        self.presentation
            .reload_all_html_artifacts(self.state.clone());
        if get_file_type(filename) == FileType::Html && !skip_wait {
            let reader = self.presentation.get_logs(filename).ok();
            result.push('\n');
            if let Some(reader) = reader {
                self.presentation.wait(Duration::from_millis(1500)).await;
                result.push_str(&reader());
            }
        }
        result
    }
    /// Delete a record and view, retaining removed records through existing aliases.
    fn delete(&self, filename: &str) -> Result<String, String> {
        self.find(filename)?;
        let artifacts = self.artifacts();
        write(&artifacts).shift_remove(filename);
        self.replace_map(&artifacts);
        self.presentation.remove_artifact_element(filename);
        let active = read(&self.state.0).active.as_deref() == Some(filename);
        if active {
            let next = read(&self.artifacts()).keys().next().cloned();
            if let Some(next) = next {
                self.show_artifact(&next);
            } else {
                write(&self.state.0).active = None;
                self.presentation.request_update();
            }
        }
        self.presentation.on_artifacts_change();
        self.presentation.request_update();
        self.presentation
            .reload_all_html_artifacts(self.state.clone());
        Ok(format!("Deleted file {filename}"))
    }
    /// Read the current view, independently of record presence.
    fn logs(&self, filename: &str) -> Result<String, String> {
        match self.presentation.get_logs(filename) {
            Ok(reader) => Ok(reader()),
            Err(ArtifactLogError::Missing) => Err(self.missing(filename)),
            Err(ArtifactLogError::NotHtml) => Err(format!(
                "Error: File {filename} is not an HTML file. Logs are only available for HTML files."
            )),
        }
    }
}
/// Admit create operands: nonempty content and filename for a file not yet present.
fn created_content(params: &ArtifactsParams, exists: bool) -> Result<&str, String> {
    let content = params.content.as_deref().unwrap_or_default();
    if content.is_empty() || params.filename.is_empty() {
        return Err("Error: create command requires filename and content".into());
    }
    if exists {
        return Err(format!("Error: File {} already exists", params.filename));
    }
    Ok(content)
}
/// Admit edit operands against the current content and return the edited content.
fn edited_content(params: &ArtifactsParams, current: &str) -> Result<String, String> {
    if matches!(params.command, ArtifactCommand::Rewrite) {
        return params
            .content
            .as_ref()
            .filter(|content| !content.is_empty())
            .cloned()
            .ok_or_else(|| "Error: rewrite command requires content".into());
    }
    let (Some(old), Some(new)) = (
        params.old_str.as_ref().filter(|old| !old.is_empty()),
        params.new_str.as_ref(),
    ) else {
        return Err("Error: update command requires old_str and new_str".into());
    };
    if !current.contains(old.as_str()) {
        return Err(format!(
            "Error: String not found in file. Here is the full content:\n\n{current}"
        ));
    }
    Ok(current.replacen(old.as_str(), new, 1))
}
/// The model-facing declaration shared by both executable tools and history replay.
fn definition<C: Default>() -> Tool<C> {
    Tool {
        name: "artifacts".into(),
        description: artifacts_tool_description(&[
            ATTACHMENTS_RUNTIME_DESCRIPTION,
            ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO,
        ]),
        parameters: schema().with_code(C::default()),
    }
}
/// Artifact view selected from the literal filename suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    /// HTML document.
    Html,
    /// Scalable vector image.
    Svg,
    /// Markdown document.
    Markdown,
    /// Raster image.
    Image,
    /// Portable document.
    Pdf,
    /// Spreadsheet.
    Excel,
    /// Word document.
    Docx,
    /// Text or source code.
    Text,
    /// Download-only file.
    Generic,
}
/// Select a view using the last literal dot-delimited segment.
#[must_use]
pub fn get_file_type(filename: &str) -> FileType {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "html" => FileType::Html,
        "svg" => FileType::Svg,
        "md" | "markdown" => FileType::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => FileType::Image,
        "pdf" => FileType::Pdf,
        "xlsx" | "xls" => FileType::Excel,
        "docx" => FileType::Docx,
        "txt" | "json" | "xml" | "yaml" | "yml" | "csv" | "js" | "ts" | "jsx" | "tsx" | "py"
        | "java" | "c" | "cpp" | "h" | "css" | "scss" | "sass" | "less" | "sh" => FileType::Text,
        _ => FileType::Generic,
    }
}
/// Model schema using the shared string-enum owner.
fn schema() -> maestro_models::ParameterSchema {
    maestro_models::ParameterSchema::authored(typed_schema(), authored_kinds())
}
/// Declared node kinds of the artifact parameters.
fn authored_kinds() -> [(String, maestro_models::SchemaKind); 5] {
    use maestro_models::SchemaKind;
    [
        (String::new(), SchemaKind::Object),
        ("/properties/filename".to_owned(), SchemaKind::String),
        ("/properties/content".to_owned(), SchemaKind::String),
        ("/properties/old_str".to_owned(), SchemaKind::String),
        ("/properties/new_str".to_owned(), SchemaKind::String),
    ]
}
/// Ordinary JSON Schema text of the artifact parameters.
fn typed_schema() -> serde_json::Value {
    json!({"type":"object", "required":["command","filename"], "properties": {
        "command": string_enum(&["create", "update", "rewrite", "get", "delete", "logs"], Some(StringEnumOptions { description: Some("The operation to perform"), default: None })),
        "filename": {"type":"string", "description":"Filename including extension (e.g., 'index.html', 'script.js')"},
        "content": {"type":"string", "description":"File content"},
        "old_str": {"type":"string", "description":"String to replace (for update command)"},
        "new_str": {"type":"string", "description":"Replacement string (for update command)"}
    }})
}
mod reconstruction;
#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
