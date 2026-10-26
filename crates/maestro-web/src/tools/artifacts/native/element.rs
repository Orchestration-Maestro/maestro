//! Bare source tags mounting the same native format owners consumed by the panel.
use super::NativeArtifact;
use crate::{
    dom::{self, DomContent as _, DomRoot, DomView, Getter, Link, Property, Reflected, Setter},
    tools::types::ToolUiError,
};
use maestro_app::browser::tools::artifacts::FileType;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};

/// Shared original fields of one source element.
pub(super) type Controller = Rc<RefCell<NativeArtifact>>;
/// Native placement retained across reconnections.
type Placement = Rc<RefCell<Option<DomRoot>>>;
/// Current caller operands shared by typed updates and native property callbacks.
type View = Rc<RefCell<DomView>>;
/// Operands passed through the existing constructor handoff.
type Creating = (Controller, Placement, View);
/// Source-tag state, with no strong link back to its native element.
struct Element {
    /// Native element reference.
    root: Link,
    /// Current format owner.
    controller: Controller,
    /// Body retained while its native tree is detached.
    mounted: Placement,
    /// Page operands of the source element.
    view: View,
    /// Content property callbacks.
    _source: (Getter, Setter),
    /// Reflected filename callbacks.
    _properties: Reflected,
}
/// Mount or update the actual format body after releasing any original-field mutation guard.
fn paint(
    root: &Link,
    controller: &Controller,
    mounted: &Placement,
    view: &DomView,
) -> Result<(), ToolUiError> {
    let Some(root) = dom::connected(root) else {
        return Ok(());
    };
    root.class_list()
        .add_1("maestro-artifact-views")
        .map_err(|error| ToolUiError::native(&error))?;
    let artifact = controller
        .try_borrow()
        .map_err(|_| ToolUiError::Presentation("Artifact is being updated".into()))?;
    let mut body = mounted.borrow_mut();
    if let Some(body) = body.as_ref() {
        artifact.update(body, view)?;
    } else {
        *body = Some(artifact.mount(view)?);
    }
    if let Some(body) = body.as_ref()
        && !body.node.is_same_node(Some(root.as_ref()))
        && !body
            .node
            .parent_node()
            .is_some_and(|parent| parent.is_same_node(Some(root.as_ref())))
    {
        root.append_child(&body.node)
            .map_err(|e| ToolUiError::native(&e))?;
    }
    Ok(())
}
impl custom_element::CustomElement for Element {
    fn connected_callback(&mut self) {
        let view = self.view.borrow().clone();
        let _ = paint(&self.root, &self.controller, &self.mounted, &view);
    }
    fn disconnected_callback(&mut self) {
        // Detaching the retained child breaks its ancestor backedge while preserving view state.
        if let Some(body) = self.mounted.borrow().as_ref() {
            body.node.unchecked_ref::<web_sys::Element>().remove();
        }
        if matches!(*self.controller.borrow(), NativeArtifact::Docx(_)) {
            self.mounted.take();
        }
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, value: JsValue) {
        self.controller
            .borrow_mut()
            .set_filename(value.as_string().unwrap_or_default());
        let view = self.view.borrow().clone();
        let _ = paint(&self.root, &self.controller, &self.mounted, &view);
    }
}
/// Release the mounted format's resources even if its element remains connected.
struct RootOwner {
    /// Mounted format resources.
    mounted: Placement,
    /// Latest caller operands read by source property updates.
    view: View,
    /// Accessors overriding a directly mounted format's body-local source property.
    source: Option<SourceBinding>,
}
impl Drop for RootOwner {
    fn drop(&mut self) {
        self.source.take();
        self.mounted.take();
    }
}
/// Restore the element's own accessors before releasing the shared-controller callbacks.
struct SourceBinding {
    /// Non-owning native element.
    root: Link,
    /// Original body-local property, still retained by the custom element.
    descriptor: js_sys::Object,
    /// Controller-backed callbacks retained for the mounted root's lifetime.
    _callbacks: (Getter, Setter),
}
impl Drop for SourceBinding {
    fn drop(&mut self) {
        if let Some(root) = dom::live(&self.root) {
            let _ = js_sys::Reflect::define_property(&root, &"content".into(), &self.descriptor);
        }
    }
}
/// Bind a directly mounted format's source property to the same controller as typed updates.
fn direct(controller: &Controller, view: &DomView) -> Result<DomRoot, ToolUiError> {
    let body = controller.borrow().mount(view)?;
    let node = body.node.unchecked_ref::<web_sys::HtmlElement>().clone();
    let descriptor = js_sys::Reflect::get_own_property_descriptor(&node, &"content".into())
        .map_err(|error| ToolUiError::native(&error))?
        .unchecked_into();
    let mounted = Rc::new(RefCell::new(Some(body)));
    let current = Rc::new(RefCell::new(view.clone()));
    let callbacks = source(&node, controller, &mounted, &current);
    let binding = SourceBinding {
        root: Link::new(&node),
        descriptor,
        _callbacks: callbacks,
    };
    Ok(DomRoot {
        node: node.into(),
        owner: Some(Box::new(RootOwner {
            mounted,
            view: current,
            source: Some(binding),
        })),
    })
}
/// Construct one source element from an already selected format owner.
pub(in crate::tools::artifacts) fn mount(
    controller: &Controller,
    view: &DomView,
) -> Result<DomRoot, ToolUiError> {
    if matches!(
        *controller.borrow(),
        NativeArtifact::Html(_) | NativeArtifact::Svg(_) | NativeArtifact::Pdf(_)
    ) {
        return direct(controller, view);
    }
    let mounted: Placement = Rc::default();
    let current = Rc::new(RefCell::new(view.clone()));
    let node = dom::create_with(
        &view.document,
        controller.borrow().tag(),
        (
            Rc::clone(controller),
            Rc::clone(&mounted),
            Rc::clone(&current),
        ),
    )
    .map_err(|e| ToolUiError::native(&e))?;
    node.set_class_name("maestro-artifact-views");
    let filename = controller.borrow().filename().to_owned();
    super::super::artifact_element::filename(&node, &filename)?;
    Ok(DomRoot {
        node: node.into(),
        owner: Some(Box::new(RootOwner {
            mounted,
            view: current,
            source: None,
        })),
    })
}
/// Install the unreflected source property using the shared property-descriptor owner.
fn source(
    root: &web_sys::HtmlElement,
    controller: &Controller,
    mounted: &Placement,
    view: &View,
) -> (Getter, Setter) {
    let (read, write) = (Rc::clone(controller), Rc::clone(controller));
    let link = Link::new(root);
    let (mounted, view) = (Rc::clone(mounted), Rc::clone(view));
    let get = Getter::new(move || read.borrow().content().into());
    let set = Setter::new(move |value: JsValue| {
        let Some(value) = value.as_string() else {
            wasm_bindgen::throw_str("Artifact content must be a string")
        };
        let result = write.borrow_mut().set_content(value);
        let view = view.borrow().clone();
        if let Err(error) = result.and_then(|()| paint(&link, &write, &mounted, &view)) {
            wasm_bindgen::throw_str(&error.to_string());
        }
    });
    let _ = dom::install(root, "content", &get, &set);
    (get, set)
}
/// Define source tags whose body lifetime is managed by their native format owner.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    for (tag, kind) in [
        ("text-artifact", FileType::Text),
        ("generic-artifact", FileType::Generic),
        ("image-artifact", FileType::Image),
        ("markdown-artifact", FileType::Markdown),
        ("excel-artifact", FileType::Excel),
        ("docx-artifact", FileType::Docx),
    ] {
        let view = Rc::clone(view);
        dom::define_element(
            tag,
            vec!["filename".into()],
            &[Property::Text("filename", "filename")],
            move |root, properties| {
                let (controller, mounted, current) = dom::take_creating::<Creating>()
                    .unwrap_or_else(|| {
                        (
                            Rc::new(RefCell::new(NativeArtifact::new(kind))),
                            Rc::default(),
                            Rc::new(RefCell::new((*view).clone())),
                        )
                    });
                let callbacks = source(&root, &controller, &mounted, &current);
                Element {
                    root: Link::new(&root),
                    controller,
                    mounted,
                    view: current,
                    _source: callbacks,
                    _properties: properties,
                }
            },
        )
        .map_err(|e| ToolUiError::native(&e))?;
    }
    Ok(())
}

/// Update the actual mounted body rather than its source-tag wrapper.
pub(in crate::tools::artifacts) fn update(
    controller: &Controller,
    root: &DomRoot,
    view: &DomView,
) -> Result<(), ToolUiError> {
    let filename = controller.borrow().filename().to_owned();
    super::super::artifact_element::filename(root.node.unchecked_ref(), &filename)?;
    if let Some(owner) = root
        .owner
        .as_ref()
        .and_then(|owner| owner.downcast_ref::<RootOwner>())
    {
        let previous = owner.view.replace(view.clone());
        drop(previous);
        if let Some(body) = owner.mounted.borrow().as_ref() {
            controller.borrow().update(body, view)?;
        }
        Ok(())
    } else {
        controller.borrow().update(root, view)
    }
}
