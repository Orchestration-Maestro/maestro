//! HTML artifact keeping its original source beside an isolated preview and its logs.
use crate::{
    I18n, SandboxIframe,
    browser::sandbox_value::BrowserSandboxValue,
    components::preview_code_toggle::ViewMode,
    dom::{DomContent, DomRoot, DomView},
    tools::{
        artifacts::console::{Console, LogEntry},
        types::ToolUiError,
    },
};
use maestro_app::{SandboxRuntimeProvider, SandboxUrlProvider, SharedHandle};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

mod body;
mod header;
pub(crate) use body::define;

/// One runtime provider, shared with the sandbox router.
type Provider = SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
/// Ordered logs shared with the mounted console.
type Logs = Rc<RefCell<Vec<LogEntry>>>;
/// The connected body's isolated frame.
type Frame = Rc<RefCell<Option<Rc<SandboxIframe>>>>;

/// Original HTML whose mounted bodies keep a preview region and a highlighted source listing.
#[derive(Default)]
pub struct HtmlArtifact {
    /// Authored filename, which also names the artifact's sandbox.
    pub filename: String,
    /// Providers whose globals and runtime each load receives, in order.
    pub runtime_providers: Vec<Provider>,
    /// Sandbox page address given to the frame before each load, when present.
    pub sandbox_url_provider: Option<SandboxUrlProvider>,
    /// The connected body's isolated frame adapter, empty while no body is connected.
    pub sandbox_iframe_ref: Frame,
    /// Original source.
    content: String,
    /// Logs of the current source, in arrival order.
    logs: Logs,
    /// Whether bodies show the preview or the source listing.
    mode: Rc<Cell<ViewMode>>,
    /// The body this artifact last mounted or updated.
    body: Rc<RefCell<Weak<body::Body>>>,
}
impl HtmlArtifact {
    /// Start with empty filename, source and providers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Borrow the original source.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
    /// Replace the original source; a changed source clears the logs and, while a body is
    /// connected and the source is not empty, loads it.
    /// # Errors
    /// Returns the reached provider, address or frame failure, after the source and logs changed.
    pub fn set_content(&mut self, content: String) -> Result<(), ToolUiError> {
        if self.content == content {
            return Ok(());
        }
        self.content = content;
        self.logs.borrow_mut().clear();
        let body = self.body.borrow().upgrade();
        if let Some(body) = body {
            body.assign(self)?;
        }
        if self.content.is_empty() {
            return Ok(());
        }
        self.execute_content(&self.content)
    }
    /// Load `html` into the mounted body's frame, even when empty; without a frame nothing runs.
    /// # Errors
    /// Returns the reached provider, address or frame failure.
    pub fn execute_content(&self, html: &str) -> Result<(), ToolUiError> {
        self.binding().execute(html)
    }
    /// The logs as `[type] text` lines, or the localized guidance naming the file when empty.
    /// # Errors
    /// Returns a localization or value formatting failure.
    pub fn get_logs(&self, i18n: &I18n) -> Result<String, ToolUiError> {
        if self.logs.borrow().is_empty() {
            let template = i18n.text("No logs for {filename}")?;
            return Ok(template.replacen("{filename}", &self.filename, 1));
        }
        let mut console = Console::new();
        console.logs = Rc::clone(&self.logs);
        console.get_logs_text()
    }
    /// The operands a load reads, sharing this artifact's handles.
    fn binding(&self) -> Binding {
        Binding {
            content: self.content.clone(),
            filename: self.filename.clone(),
            providers: self.runtime_providers.clone(),
            url: self.sandbox_url_provider.clone(),
            frame: Rc::clone(&self.sandbox_iframe_ref),
            logs: Rc::clone(&self.logs),
            mode: Rc::clone(&self.mode),
            body: self.body.borrow().clone(),
        }
    }
}
/// Operands of one load, which a mounted body keeps from the artifact that last updated it.
#[derive(Clone)]
struct Binding {
    /// Current controller source, independent of presentation updates.
    content: String,
    /// Filename naming the sandbox.
    filename: String,
    /// Runtime providers in load order.
    providers: Vec<Provider>,
    /// Sandbox page address, when configured.
    url: Option<SandboxUrlProvider>,
    /// Published frame of the artifact.
    frame: Frame,
    /// Logs receiving console messages.
    logs: Logs,
    /// Presentation the body shows.
    mode: Rc<Cell<ViewMode>>,
    /// Body repainting its console after each message.
    body: Weak<body::Body>,
}
impl Binding {
    /// Load `html` with the completion signal into the published frame, if any.
    fn execute(&self, html: &str) -> Result<(), ToolUiError> {
        let Some(sandbox) = self.frame.borrow().clone() else {
            return Ok(());
        };
        // An absent address keeps the one the frame already has.
        if let Some(url) = &self.url {
            let replaced = sandbox.sandbox_url_provider.replace(Some(url.clone()));
            drop(replaced);
        }
        let consumer = body::LogConsumer::shared(&self.logs, &self.body);
        sandbox
            .load_content(
                &sandbox_id(&self.filename),
                &complete_html(html),
                self.providers.clone(),
                vec![consumer],
            )
            .map_err(|error| ToolUiError::Presentation(error.message))
    }
}
/// The sandbox identifier of an artifact named `filename`.
fn sandbox_id(filename: &str) -> String {
    format!("artifact-{filename}")
}
/// Append the completion signal after the whole source, which the page parser then places.
fn complete_html(html: &str) -> String {
    format!("{html}<script>if (window.complete) window.complete();</script>")
}
impl DomContent for HtmlArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        body::mount(self, view)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        use wasm_bindgen::JsCast as _;
        crate::tools::artifacts::artifact_element::filename(
            root.node.unchecked_ref(),
            &self.filename,
        )?;
        root.owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<body::Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("HTML body is not mounted".to_owned()))?
            .0
            .bind(self, view)
    }
}
