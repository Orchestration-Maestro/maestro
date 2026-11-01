//! Connection-scoped progressive PDF pages using the shared document owner.
use super::PdfArtifact;
use crate::{
    browser::{
        attachment_content::browser::{append, element, sync_text},
        timer::{self, Timer},
    },
    dialogs::attachment_overlay::{
        error,
        pdf::{self, Pdf},
    },
    dom::{self, DomContent, DomRoot, DomView, Getter, Link, Property, Reflected, Setter},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// One native PDF's original source, completed page nodes and pending delivery.
struct State {
    /// Root containing pages or a literal diagnostic.
    node: web_sys::Element,
    /// Page and localization operands.
    view: RefCell<DomView>,
    /// Original encoded source.
    source: RefCell<String>,
    /// Last explicit assignment.
    assignment: RefCell<Option<Rc<()>>>,
    /// Document while progressive pages remain.
    pdf: RefCell<Pdf>,
    /// Next native page delivery, cancelled on replacement or disconnect.
    pending: RefCell<Option<Timer>>,
    /// Whether the page admits document work.
    connected: Cell<bool>,
    /// Whether every page finished successfully.
    complete: Cell<bool>,
    /// Current localized error heading, absent after success.
    failure: RefCell<Option<web_sys::Element>>,
}
impl State {
    /// Create a detached document body without opening a reader.
    fn new(view: &DomView, content: String) -> Result<Rc<Self>, ToolUiError> {
        Ok(Rc::new(Self {
            node: element(&view.document, "section", "maestro-pdf-artifact")?,
            view: RefCell::new(view.clone()),
            source: RefCell::new(content),
            assignment: RefCell::default(),
            pdf: RefCell::default(),
            pending: RefCell::default(),
            connected: Cell::new(false),
            complete: Cell::new(false),
            failure: RefCell::default(),
        }))
    }
    /// Update the source, retrying equal failed assignments but retaining successful pages.
    fn assign(self: &Rc<Self>, content: String, requested: bool) -> Result<(), ToolUiError> {
        if *self.source.borrow() == content && !(self.failure.borrow().is_some() && requested) {
            if let Some(heading) = self.failure.borrow().as_ref() {
                heading.set_text_content(Some(&self.view.borrow().i18n.text("Error loading PDF")?));
            }
            return Ok(());
        }
        *self.source.borrow_mut() = content;
        let pending = self.pending.take();
        drop(pending);
        let retain = self.source.borrow().is_empty() && self.failure.borrow().is_none();
        self.pdf.borrow_mut().reset(retain);
        self.complete.set(retain);
        if !retain {
            self.node.replace_children_with_node_0();
        }
        self.failure.take();
        if self.connected.get() && !self.source.borrow().is_empty() {
            self.start()?;
        }
        Ok(())
    }
    /// Open the current bytes and schedule pages, keeping errors in the body.
    fn start(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let prepared = crate::utils::base64::original(&self.source.borrow())
            .map_err(|error| error.to_string())
            .and_then(|bytes| self.pdf.borrow_mut().open(bytes));
        if let Err(error) = prepared {
            return self.error(&error);
        }
        let root = self.pdf.borrow().mount_root(&self.view.borrow(), true)?;
        self.node.replace_children_with_node_0();
        self.node
            .append_child(&root.node)
            .map_err(|e| ToolUiError::native(&e))?;
        self.complete.set(false);
        self.schedule()
    }
    /// Complete one shared-reader page before requesting the next page's native frame.
    fn advance(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let result = self.pdf.borrow_mut().next_page();
        match result {
            Ok(remains) => {
                self.pdf.borrow().sync_root()?;
                if remains {
                    self.schedule()?;
                } else {
                    self.complete.set(true);
                }
                Ok(())
            }
            Err(error) => self.error(&error),
        }
    }
    /// Schedule through the existing cancel-on-drop callback owner.
    fn schedule(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let window = self
            .view
            .borrow()
            .document
            .default_view()
            .ok_or_else(|| ToolUiError::Presentation("Missing PDF window".into()))?;
        let weak = Rc::downgrade(self);
        let frame = timer::frame(
            window,
            Rc::new(move || {
                if let Some(state) = weak.upgrade().filter(|state| state.connected.get()) {
                    // A page callback reports reader failures in its own native body.
                    let _ = state.advance();
                }
            }),
        )
        .map_err(|e| ToolUiError::native(&e))?;
        let previous = self.pending.replace(Some(frame));
        drop(previous);
        Ok(())
    }
    /// Publish the shared literal diagnostic and release the unfinished reader.
    fn error(&self, message: &str) -> Result<(), ToolUiError> {
        self.pdf.borrow_mut().reset(false);
        let view = self.view.borrow();
        let (panel, nodes) = error::nodes(&view.document, true)?;
        sync_text(
            Some(&nodes),
            &[
                view.i18n.text("Error loading PDF")?,
                pdf::detail(message, &*view.i18n)?,
            ],
        );
        self.node.replace_children_with_node_0();
        append(&self.node, &panel)?;
        *self.failure.borrow_mut() = Some(nodes[0].clone());
        Ok(())
    }
    /// Admit unfinished rendering when connected, preserving completed page identity.
    fn connect(self: &Rc<Self>) -> Result<(), ToolUiError> {
        self.connected.set(true);
        if !self.complete.get()
            && self.failure.borrow().is_none()
            && !self.source.borrow().is_empty()
        {
            self.start()?;
        }
        Ok(())
    }
    /// Cancel the pending page and release the reader without discarding completed images.
    fn disconnect(&self) {
        self.connected.set(false);
        let pending = self.pending.take();
        drop(pending);
        self.pdf.borrow_mut().reset(true);
    }
}
/// Typed-root share of a PDF body.
struct Mounted(Rc<State>);
impl Drop for Mounted {
    fn drop(&mut self) {
        self.0.disconnect();
    }
}
impl PdfArtifact {
    /// Build a complete original-byte PDF download independently of rendering.
    /// # Errors
    /// Returns decoding, localization or native failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::download(
            view,
            &self.filename,
            "application/pdf",
            &crate::utils::base64::original(&self.content)
                .map_err(|e| ToolUiError::Presentation(e.to_string()))?,
        )
    }
}
impl DomContent for PdfArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let state = State::new(view, self.content.clone())?;
        *state.assignment.borrow_mut() = Some(Rc::clone(&self.assignment));
        let node = dom::create_with(&view.document, "pdf-artifact", Rc::downgrade(&state))
            .map_err(|e| ToolUiError::native(&e))?;
        node.set_class_name("maestro-artifact-views");
        crate::tools::artifacts::artifact_element::filename(&node, &self.filename)?;
        append(&node, &state.node)?;
        Ok(DomRoot {
            node: node.into(),
            owner: Some(Box::new(Mounted(state))),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        use wasm_bindgen::JsCast as _;
        crate::tools::artifacts::artifact_element::filename(
            root.node.unchecked_ref(),
            &self.filename,
        )?;
        let state = &root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("PDF body is not mounted".into()))?
            .0;
        *state.view.borrow_mut() = view.clone();
        let requested = state
            .assignment
            .borrow()
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &self.assignment));
        *state.assignment.borrow_mut() = Some(Rc::clone(&self.assignment));
        state.assign(self.content.clone(), requested)
    }
}
/// Native PDF lifecycle, with default body ownership while connected.
struct Element {
    /// Weak element whose child placement this state manages.
    root: Link,
    /// Page operands for bare construction.
    view: Rc<DomView>,
    /// Body passed by a typed mount or created on first connection.
    state: Rc<RefCell<Weak<State>>>,
    /// Original source retained before first connection.
    source: Rc<RefCell<String>>,
    /// Source property callbacks.
    _source: (Getter, Setter),
    /// Filename property callbacks.
    _properties: Reflected,
    /// Default body retained while connected.
    owned: Option<Rc<State>>,
}
impl custom_element::CustomElement for Element {
    fn connected_callback(&mut self) {
        let Some(root) = dom::live(&self.root) else {
            return;
        };
        let _ = root.class_list().add_1("maestro-artifact-views");
        let state = self.state.borrow().upgrade();
        let state = state.or_else(|| {
            let state = State::new(&self.view, self.source.borrow().clone()).ok()?;
            root.replace_children_with_node_0();
            append(&root, &state.node).ok()?;
            *self.state.borrow_mut() = Rc::downgrade(&state);
            self.owned = Some(Rc::clone(&state));
            Some(state)
        });
        if let Some(state) = state {
            let _ = append(&root, &state.node);
            let _ = state.connect();
        }
    }
    fn disconnected_callback(&mut self) {
        if let Some(state) = self.state.borrow().upgrade() {
            state.disconnect();
        }
        if self.owned.is_some()
            && let Some(state) = self.state.borrow().upgrade()
        {
            state.node.remove();
        }
    }
}
/// Define the source-owned PDF element through the shared registration owner.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let view = Rc::clone(view);
    dom::define_element(
        "pdf-artifact",
        vec!["filename".into()],
        &[Property::Text("filename", "filename")],
        move |root, properties| {
            let state = Rc::new(RefCell::new(
                dom::take_creating::<Weak<State>>().unwrap_or_default(),
            ));
            let initial = state
                .borrow()
                .upgrade()
                .map_or_else(String::new, |state| state.source.borrow().clone());
            let source = Rc::new(RefCell::new(initial));
            let (read, write) = (Rc::clone(&source), Rc::clone(&source));
            let (reading, writing) = (Rc::clone(&state), Rc::clone(&state));
            let get = Getter::new(move || {
                reading
                    .borrow()
                    .upgrade()
                    .map_or_else(
                        || read.borrow().clone(),
                        |state| state.source.borrow().clone(),
                    )
                    .into()
            });
            let set = Setter::new(move |value: wasm_bindgen::JsValue| {
                let Some(value) = value.as_string() else {
                    wasm_bindgen::throw_str("PDF content must be a string")
                };
                write.borrow_mut().clone_from(&value);
                let state = writing.borrow().upgrade();
                if let Some(state) = state
                    && let Err(error) = state.assign(value, true)
                {
                    wasm_bindgen::throw_str(&error.to_string());
                }
            });
            let _ = dom::install(&root, "content", &get, &set);
            Element {
                root: Link::new(&root),
                view: Rc::clone(&view),
                state,
                source,
                owned: None,
                _source: (get, set),
                _properties: properties,
            }
        },
    )
    .map_err(|e| ToolUiError::native(&e))
}
