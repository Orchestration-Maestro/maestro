//! Connection-owned inert SVG resources beside a retained original-source listing.
use super::SvgArtifact;
use crate::{
    browser::attachment_content::browser::append,
    dom::{self, DomContent, DomRoot, DomView, Getter, Link, Property, Reflected, Setter},
    tools::{
        artifacts::{
            artifact_element::{
                ArtifactElement as _,
                preview::{self, Regions},
            },
            image_artifact::ImageArtifact,
            text_artifact::{SourceProfile, TextArtifact},
        },
        types::ToolUiError,
    },
    utils::download::ObjectUrl,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue};

/// Original fields and resources of one placed SVG document.
struct State {
    /// Source listing controller, independent of retained surfaces.
    source: RefCell<TextArtifact>,
    /// Image controller using an inert image resource, never inline document markup.
    image: RefCell<ImageArtifact>,
    /// Native placements kept through mode changes.
    regions: Rc<Regions>,
    /// Current document and localization operands.
    view: RefCell<DomView>,
    /// Revoked on replacement, disconnection or root release.
    url: RefCell<Option<ObjectUrl>>,
    /// Whether native connection admits a resource.
    connected: Cell<bool>,
}
impl State {
    /// Build both placements without allocating a disconnected image resource.
    fn new(artifact: &SvgArtifact, view: &DomView) -> Result<Rc<Self>, ToolUiError> {
        let mut source = TextArtifact::page_profile(SourceProfile::Svg);
        source.set_filename(artifact.source.filename().into());
        source.set_content(artifact.source.content().into());
        let mut image = ImageArtifact::page_svg();
        image.set_filename(artifact.source.filename().into());
        let regions = Regions::mount(view, &image, &source, &artifact.toggle)?;
        regions.show(artifact.toggle.mode())?;
        preview::publish(&artifact.body, &regions);
        regions.bind(&artifact.toggle, view)?;
        Ok(Rc::new(Self {
            source: RefCell::new(source),
            image: RefCell::new(image),
            regions,
            view: RefCell::new(view.clone()),
            url: RefCell::default(),
            connected: Cell::new(false),
        }))
    }
    /// Update original source and only replace the resource after an actual source edit.
    fn content(&self, content: String) -> Result<(), ToolUiError> {
        if self.source.borrow().content() == content {
            return Ok(());
        }
        self.source.borrow_mut().set_content(content);
        if self.connected.get() {
            self.resource()?;
        }
        self.paint()
    }
    /// Apply the image and listing controllers to their stable roots.
    fn paint(&self) -> Result<(), ToolUiError> {
        self.regions.update(
            &self.view.borrow(),
            &*self.image.borrow(),
            &*self.source.borrow(),
        )
    }
    /// Replace the one image URL using the original UTF-8 bytes.
    fn resource(&self) -> Result<(), ToolUiError> {
        let previous = self.url.take();
        drop(previous);
        let source = self.source.borrow().content().to_owned();
        let url = (!source.is_empty())
            .then(|| ObjectUrl::new(source.as_bytes(), "image/svg+xml"))
            .transpose()?;
        self.image.borrow_mut().set_content(
            url.as_ref()
                .map_or_else(String::new, |url| url.as_str().into()),
        );
        *self.url.borrow_mut() = url;
        Ok(())
    }
    /// Reconstruct the image resource once per native connection.
    fn connect(&self) -> Result<(), ToolUiError> {
        if !self.connected.replace(true) {
            self.resource()?;
        }
        self.paint()
    }
    /// Withdraw the resource while retaining original source and native placements.
    fn disconnect(&self) {
        self.connected.set(false);
        let previous = self.url.take();
        drop(previous);
        self.image.borrow_mut().set_content(String::new());
        // Disconnection has no error channel; a failed paint cannot retain the revoked resource.
        let _ = self.paint();
    }
}
/// Typed-root ownership, independent of whether the caller still retains its element.
struct Mounted(Rc<State>);
impl Drop for Mounted {
    fn drop(&mut self) {
        self.0.disconnect();
    }
}
impl SvgArtifact {
    /// Build preview/source, copy and original SVG download controls.
    /// # Errors
    /// Returns localization or native control failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        preview::header(
            view,
            self.source.get_header_buttons(view)?,
            &self.toggle,
            &self.body,
        )
    }
}
impl DomContent for SvgArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let state = State::new(self, view)?;
        let node = dom::create_with(&view.document, "svg-artifact", Rc::downgrade(&state))
            .map_err(|e| ToolUiError::native(&e))?;
        node.set_class_name("maestro-artifact-views");
        crate::tools::artifacts::artifact_element::filename(&node, self.source.filename())?;
        append(&node, &state.regions.node)?;
        Ok(DomRoot {
            node: node.into(),
            owner: Some(Box::new(Mounted(state))),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        crate::tools::artifacts::artifact_element::filename(
            root.node.unchecked_ref(),
            self.source.filename(),
        )?;
        let state = &root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("SVG body is not mounted".into()))?
            .0;
        preview::publish(&self.body, &state.regions);
        state.regions.bind(&self.toggle, view)?;
        *state.view.borrow_mut() = view.clone();
        state
            .source
            .borrow_mut()
            .set_filename(self.source.filename().into());
        state
            .image
            .borrow_mut()
            .set_filename(self.source.filename().into());
        state.content(self.source.content().into())?;
        state.paint()?;
        state.regions.show(self.toggle.mode())
    }
}
/// One autonomous SVG element with unreflected source and connection-scoped default body.
struct Element {
    /// Weak native root.
    root: Link,
    /// Default page operands.
    view: Rc<DomView>,
    /// Body provided by a typed mount or created by a bare element.
    state: Rc<RefCell<Weak<State>>>,
    /// Default body retained only during connection.
    owned: Option<Rc<State>>,
    /// Unreflected original source, retained while disconnected.
    content: Rc<RefCell<String>>,
    /// Native property callables.
    _content: (Getter, Setter),
    /// Reflected filename callables.
    _properties: Reflected,
}
impl custom_element::CustomElement for Element {
    fn connected_callback(&mut self) {
        let Some(root) = dom::live(&self.root) else {
            return;
        };
        let _ = root.class_list().add_1("maestro-artifact-views");
        let state = self.state.borrow().upgrade();
        let state = state.or_else(|| {
            let mut artifact = SvgArtifact::default();
            artifact.set_filename(root.get_attribute("filename").unwrap_or_default());
            artifact.set_content(self.content.borrow().clone());
            let state = State::new(&artifact, &self.view).ok()?;
            root.replace_children_with_node_0();
            append(&root, &state.regions.node).ok()?;
            *self.state.borrow_mut() = Rc::downgrade(&state);
            self.owned = Some(Rc::clone(&state));
            Some(state)
        });
        if let Some(state) = state {
            let _ = state.connect();
        }
    }
    fn disconnected_callback(&mut self) {
        if let Some(state) = self.state.borrow().upgrade() {
            *self.content.borrow_mut() = state.source.borrow().content().into();
            state.disconnect();
        }
        self.owned = None;
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, value: JsValue) {
        if let Some(state) = self.state.borrow().upgrade() {
            let filename = value.as_string().unwrap_or_default();
            state.source.borrow_mut().set_filename(filename.clone());
            state.image.borrow_mut().set_filename(filename);
            let _ = state.paint();
        }
    }
}
/// Register the source-owned SVG tag using the shared native constructor and property owners.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let view = Rc::clone(view);
    dom::define_element(
        "svg-artifact",
        vec!["filename".into()],
        &[Property::Text("filename", "filename")],
        move |root, properties| {
            let state = Rc::new(RefCell::new(
                dom::take_creating::<Weak<State>>().unwrap_or_default(),
            ));
            let content = Rc::new(RefCell::new(String::new()));
            let (read, write) = (Rc::clone(&content), Rc::clone(&content));
            let (reading, writing) = (Rc::clone(&state), Rc::clone(&state));
            let get = Getter::new(move || {
                reading
                    .borrow()
                    .upgrade()
                    .map_or_else(
                        || read.borrow().clone(),
                        |state| state.source.borrow().content().into(),
                    )
                    .into()
            });
            let set = Setter::new(move |value: JsValue| {
                let value = value.as_string().unwrap_or_default();
                write.borrow_mut().clone_from(&value);
                let state = writing.borrow().upgrade();
                if let Some(state) = state {
                    let _ = state.content(value);
                }
            });
            let _ = dom::install(&root, "content", &get, &set);
            Element {
                root: Link::new(&root),
                view: Rc::clone(&view),
                state,
                owned: None,
                content,
                _content: (get, set),
                _properties: properties,
            }
        },
    )
    .map_err(|e| ToolUiError::native(&e))
}
