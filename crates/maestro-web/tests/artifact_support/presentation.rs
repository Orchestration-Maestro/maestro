//! Native local application-to-view adapter used only by composition tests.
use maestro_app::browser::tools::artifacts::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanelState,
};
use maestro_web::{ArtifactElement, tools::artifacts::create_artifact_element};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Actual retained generic view; no application command policy lives here.
pub struct Presentation {
    /// Actual native effect host.
    pub host: Rc<dyn maestro_web::browser::attachment_content::AttachmentHost>,
    /// Optional real multi-view owner, with no application backedge.
    pub panel: Option<Rc<maestro_web::ArtifactsPanel>>,
    /// Retained real artifact view.
    pub view: RefCell<Option<Box<dyn ArtifactElement>>>,
    /// Factory creation count.
    pub created: Cell<usize>,
    /// Open notifications.
    pub opened: Cell<usize>,
    /// Mutation notifications.
    pub changed: Cell<usize>,
}
impl Presentation {
    /// Retain native effects for the existing single-view composition.
    #[must_use]
    pub fn new(host: Rc<dyn maestro_web::browser::attachment_content::AttachmentHost>) -> Self {
        Self {
            host,
            panel: None,
            view: RefCell::default(),
            created: Cell::default(),
            opened: Cell::default(),
            changed: Cell::default(),
        }
    }
}
impl ArtifactPresentation for Presentation {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(0).unwrap()
    }
    fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
        _: ArtifactsPanelState,
    ) {
        if let Some(panel) = &self.panel {
            panel
                .get_or_create_artifact_element(filename, content)
                .unwrap();
            return;
        }
        let mut view = self.view.borrow_mut().take();
        if let Some(view) = &mut view {
            view.set_content(content.into());
        } else {
            view = Some(create_artifact_element(filename, content, self.host.clone()).unwrap());
            self.created.set(self.created.get() + 1);
        }
        *self.view.borrow_mut() = view;
    }
    fn show_artifact(&self, filename: &str) {
        if let Some(panel) = &self.panel {
            panel.show_artifact(filename).unwrap();
        }
    }
    fn remove_artifact_element(&self, filename: &str) {
        if let Some(panel) = &self.panel {
            panel.remove_artifact_element(filename);
            return;
        }
        let view = self.view.borrow_mut().take();
        drop(view);
    }
    fn reload_all_html_artifacts(&self, _: ArtifactsPanelState) {}
    fn get_logs(&self, _: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        Err(ArtifactLogError::NotHtml)
    }
    fn wait(&self, _: std::time::Duration) -> maestro_app::BoxFuture<()> {
        Box::pin(std::future::ready(()))
    }
    fn request_update(&self) {}
    fn on_open(&self) {
        self.opened.set(self.opened.get() + 1);
    }
    fn on_artifacts_change(&self) {
        self.changed.set(self.changed.get() + 1);
    }
}
/// Store a literal fixture record without creating its specialized view.
#[cfg(target_arch = "wasm32")]
pub(crate) fn store(
    panel: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    filename: &str,
) {
    let now = chrono::DateTime::from_timestamp_millis(0).unwrap();
    panel.artifacts().write().unwrap().insert(
        filename.into(),
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_app::browser::tools::artifacts::Artifact {
                filename: filename.into(),
                content: "stored".into(),
                created_at: now,
                updated_at: now,
            },
        )),
    );
}
/// Drive immediate tool work without a second executor implementation.
/// # Panics
/// Panics if the command unexpectedly suspends.
pub fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let std::task::Poll::Ready(value) = future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    else {
        panic!("unexpected suspended command")
    };
    value
}
