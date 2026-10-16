//! Native local application-to-view adapter used only by composition tests.
use maestro_app::browser::tools::artifacts::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, Artifacts,
};
use maestro_web::{ArtifactElement, tools::artifacts::create_artifact_element};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Actual retained generic view; no application command policy lives here.
pub struct Presentation {
    /// Actual native effect host.
    pub host: Rc<super::super::session_header_support::Host>,
    /// Retained real artifact view.
    pub view: RefCell<Option<Box<dyn ArtifactElement>>>,
    /// Factory creation count.
    pub created: Cell<usize>,
    /// Open notifications.
    pub opened: Cell<usize>,
    /// Mutation notifications.
    pub changed: Cell<usize>,
}
impl ArtifactPresentation for Presentation {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(0).unwrap()
    }
    fn get_or_create_artifact_element(&self, filename: &str, content: &str, _: Artifacts) {
        let mut view = self.view.borrow_mut().take();
        if let Some(view) = &mut view {
            view.set_content(content.into());
        } else {
            view = Some(create_artifact_element(filename, content, self.host.clone()).unwrap());
            self.created.set(self.created.get() + 1);
        }
        *self.view.borrow_mut() = view;
    }
    fn show_artifact(&self, _: &str) {}
    fn remove_artifact_element(&self, _: &str) {
        let view = self.view.borrow_mut().take();
        drop(view);
    }
    fn reload_all_html_artifacts(&self, _: Artifacts) {}
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
