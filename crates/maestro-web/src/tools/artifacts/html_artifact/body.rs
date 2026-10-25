//! The `html-artifact` element: its preview frame, console and source listing.
use super::{Binding, Frame, HtmlArtifact, Logs};
use crate::{
    SandboxIframe,
    browser::{
        attachment_content::browser::{append, element, host_error},
        sandbox_value::BrowserSandboxValue,
    },
    components::sandbox::runtime_message_router::MessageConsumer,
    dom::{DomContent as _, DomRoot, DomView, Link, Property, Reflected},
    tools::{
        artifacts::{
            console::{Console, LogEntry, LogType},
            text_artifact::source_nodes,
        },
        types::ToolUiError,
    },
};
use maestro_app::{BoxFuture, DiagnosticErrorInfo, SharedHandle};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// Element tag of a mounted HTML artifact.
const TAG: &str = "html-artifact";
/// The filename property, reflected onto its observed attribute.
const PROPERTIES: &[Property] = &[Property::Text("filename", "filename")];

/// Nodes, operands and frame of one element, shared by the element and the root that mounted it.
pub(super) struct Body {
    /// Page operands of the last mount or update.
    view: RefCell<DomView>,
    /// Operands of the artifact that last mounted or updated this body.
    binding: RefCell<Binding>,
    /// Region holding the frame and the console.
    preview: Element,
    /// Region holding the source listing.
    source: Element,
    /// Placement of the isolated frame.
    frame_parent: web_sys::HtmlElement,
    /// Highlighted listing node, kept across edits.
    code: Element,
    /// Source last written into the listing, which a connection loads.
    shown: RefCell<String>,
    /// The frame adapter, created on first connection and kept until release.
    sandbox: RefCell<Option<Rc<SandboxIframe>>>,
    /// Console painting the bound logs.
    console: RefCell<Console>,
    /// Mounted console while there are logs.
    console_root: RefCell<Option<DomRoot>>,
}
impl Body {
    /// Build detached nodes bound to `artifact`.
    fn new(artifact: &HtmlArtifact, view: &DomView) -> Result<Rc<Self>, ToolUiError> {
        let document = &view.document;
        let preview = element(document, "div", "maestro-html-preview")?;
        let frame_parent: web_sys::HtmlElement = element(document, "div", "maestro-html-frame")?
            .dyn_into()
            .map_err(|error| ToolUiError::native(&error))?;
        append(&preview, &frame_parent)?;
        let source = element(document, "div", "maestro-html-source")?;
        source
            .set_attribute("hidden", "")
            .map_err(|error| ToolUiError::native(&error))?;
        let (listing, code) = source_nodes(document)?;
        code.set_class_name("hljs language-html");
        append(&source, &listing)?;
        let body = Rc::new(Self {
            view: RefCell::new(view.clone()),
            binding: RefCell::new(artifact.binding()),
            preview,
            source,
            frame_parent,
            code,
            shown: RefCell::default(),
            sandbox: RefCell::default(),
            console: RefCell::default(),
            console_root: RefCell::default(),
        });
        body.bind(artifact, view)?;
        Ok(body)
    }
    /// Place the regions inside the element.
    fn attach(&self, node: &Element) -> Result<(), ToolUiError> {
        node.set_class_name("maestro-html-artifact");
        append(node, &self.preview)?;
        append(node, &self.source)?;
        Ok(())
    }
    /// Take `artifact`'s operands, moving the frame to it, then refresh the listing and console.
    pub(super) fn bind(
        self: &Rc<Self>,
        artifact: &HtmlArtifact,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        artifact.body.replace(Rc::downgrade(self));
        let next = artifact.binding();
        let previous = self.binding.replace(next);
        if let Some(sandbox) = self.sandbox.borrow().as_ref()
            && !Rc::ptr_eq(&previous.frame, &artifact.sandbox_iframe_ref)
            && unpublish(&previous.frame, sandbox)
        {
            let replaced = artifact
                .sandbox_iframe_ref
                .replace(Some(Rc::clone(sandbox)));
            drop(replaced);
        }
        drop(previous);
        self.console.borrow_mut().logs = Rc::clone(&artifact.logs);
        self.view.replace(view.clone());
        self.list(&artifact.content, view);
        self.paint_logs()
    }
    /// Re-list the source only when it changed, keeping the listing nodes and selection otherwise.
    fn list(&self, content: &str, view: &DomView) {
        if *self.shown.borrow() == content {
            return;
        }
        match maestro_theme::highlight_html(&*view.highlighter, content, Some("html")) {
            Ok(Some(html)) => self.code.set_inner_html(&html),
            _ => self.code.set_text_content(Some(content)),
        }
        content.clone_into(&mut self.shown.borrow_mut());
    }
    /// Show the console below the frame while there are logs.
    fn paint_logs(&self) -> Result<(), ToolUiError> {
        let console = self.console.borrow();
        let mut mounted = self.console_root.borrow_mut();
        if console.logs.borrow().is_empty() {
            if let Some(root) = mounted.take() {
                root.node.unchecked_ref::<Element>().remove();
            }
            return Ok(());
        }
        let view = self.view.borrow().clone();
        if let Some(root) = mounted.as_ref() {
            return console.update(root, &view);
        }
        let root = console.mount(&view)?;
        self.preview
            .append_child(&root.node)
            .map_err(|error| ToolUiError::native(&error))?;
        *mounted = Some(root);
        Ok(())
    }
    /// Publish the frame adapter, created on first connection, then load the listed source when
    /// it is not empty.
    fn connect(&self) -> Result<(), ToolUiError> {
        let existing = self.sandbox.borrow().clone();
        let sandbox = if let Some(sandbox) = existing {
            sandbox
        } else {
            let view = self.view.borrow().clone();
            let presentation = view
                .pack
                .presentation(view.mode)
                .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
            let sandbox = SandboxIframe::new(self.frame_parent.clone(), &presentation)
                .map_err(|error| ToolUiError::Presentation(error.message))?;
            Rc::clone(self.sandbox.borrow_mut().insert(Rc::new(sandbox)))
        };
        let replaced = self.binding.borrow().frame.replace(Some(sandbox));
        drop(replaced);
        let binding = self.binding.borrow().clone();
        let content = self.shown.borrow().clone();
        if content.is_empty() {
            return Ok(());
        }
        binding.execute(&content)
    }
    /// Withdraw the adapter from the bound artifact and remove its frame and route, keeping the
    /// adapter for a later connection.
    fn disconnect(&self) {
        let sandbox = self.sandbox.borrow().clone();
        if let Some(sandbox) = sandbox {
            unpublish(&self.binding.borrow().frame, &sandbox);
            // A failed removal keeps its registrations, which the next load retries.
            let _ = sandbox.remove();
        }
    }
    /// Disconnect, then drop the adapter.
    fn release(&self) {
        self.disconnect();
        let released = self.sandbox.take();
        drop(released);
    }
}
/// Clear `frame` when it publishes `sandbox`, reporting whether it did.
fn unpublish(frame: &Frame, sandbox: &Rc<SandboxIframe>) -> bool {
    let published = frame
        .borrow()
        .as_ref()
        .is_some_and(|live| Rc::ptr_eq(live, sandbox));
    if published {
        let removed = frame.take();
        drop(removed);
    }
    published
}
/// The mounted root's share of its body, which releases the frame when the root is dropped.
pub(super) struct Mounted(Rc<Body>);
impl Mounted {
    /// Bind the body to `artifact`.
    pub(super) fn bind(&self, artifact: &HtmlArtifact, view: &DomView) -> Result<(), ToolUiError> {
        self.0.bind(artifact, view)
    }
}
impl Drop for Mounted {
    fn drop(&mut self) {
        self.0.release();
    }
}
/// Mount `artifact` as an `html-artifact` element owning its body.
pub(super) fn mount(artifact: &HtmlArtifact, view: &DomView) -> Result<DomRoot, ToolUiError> {
    let body = Body::new(artifact, view)?;
    let node = crate::dom::create_with(&view.document, TAG, Rc::clone(&body))
        .map_err(|error| ToolUiError::native(&error))?;
    body.attach(&node)?;
    Ok(DomRoot {
        node: node.into(),
        owner: Some(Box::new(Mounted(body))),
    })
}
/// Appends each console message to the logs, then repaints the body.
pub(super) struct LogConsumer {
    /// Logs receiving the messages.
    logs: Logs,
    /// Body showing the logs.
    body: Weak<Body>,
}
impl LogConsumer {
    /// A shared consumer appending to `logs` and repainting `body`.
    pub(super) fn shared(
        logs: &Logs,
        body: &Weak<Body>,
    ) -> SharedHandle<dyn MessageConsumer<BrowserSandboxValue>> {
        SharedHandle::new(Self {
            logs: Rc::clone(logs),
            body: Weak::clone(body),
        })
    }
}
impl MessageConsumer<BrowserSandboxValue> for LogConsumer {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let field = |name: &str| {
            js_sys::Reflect::get(&message.0, &name.into()).unwrap_or(JsValue::UNDEFINED)
        };
        if field("type").as_string().as_deref() == Some("console") {
            let r#type = if field("method").as_string().as_deref() == Some("error") {
                LogType::Error
            } else {
                LogType::Log
            };
            let text = BrowserSandboxValue(field("text"));
            self.logs.borrow_mut().push(LogEntry { r#type, text });
            if let Some(body) = self.body.upgrade() {
                // The message is already recorded; a failed repaint keeps the previous console.
                let _ = body.paint_logs();
            }
        }
        Box::pin(async { Ok(()) })
    }
}
/// One `html-artifact` element.
struct ArtifactElement {
    /// Element owning this state.
    root: Link,
    /// Page operands for an element created without a body.
    view: Rc<DomView>,
    /// Body handed over by [`mount`], or built on first connection.
    body: Option<Rc<Body>>,
    /// Accessors of the filename property.
    _properties: Reflected,
}
impl ArtifactElement {
    /// The element's body, building an empty one named by its attribute on first use.
    fn body(&mut self) -> Result<Rc<Body>, ToolUiError> {
        if let Some(body) = &self.body {
            return Ok(Rc::clone(body));
        }
        let root = crate::dom::live(&self.root)
            .ok_or_else(|| ToolUiError::Presentation("HTML element was released".to_owned()))?;
        let artifact = HtmlArtifact {
            filename: root.get_attribute("filename").unwrap_or_default(),
            ..HtmlArtifact::default()
        };
        let body = Body::new(&artifact, &self.view)?;
        body.attach(&root)?;
        Ok(Rc::clone(self.body.insert(body)))
    }
}
impl custom_element::CustomElement for ArtifactElement {
    fn connected_callback(&mut self) {
        // Lifecycle callbacks have no error channel; a failed load leaves the frame empty.
        let _ = self.body().and_then(|body| body.connect());
    }
    fn disconnected_callback(&mut self) {
        if let Some(body) = &self.body {
            body.disconnect();
        }
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, new: JsValue) {
        if let Some(body) = &self.body {
            body.binding.borrow_mut().filename = new.as_string().unwrap_or_default();
        }
    }
}
/// Define the `html-artifact` element once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let view = Rc::clone(view);
    let observed = vec!["filename".to_owned()];
    crate::dom::define_element(TAG, observed, PROPERTIES, move |root, properties| {
        ArtifactElement {
            root: Link::new(&root),
            view: Rc::clone(&view),
            body: crate::dom::take_creating::<Rc<Body>>(),
            _properties: properties,
        }
    })
    .map_err(|error| host_error(&error).into())
}
