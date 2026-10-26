//! Native preview/source control group and its page element.
use super::{PreviewCodeToggle, ViewMode};
use crate::{
    browser::attachment_content::AttachmentHostError,
    components::mode_toggle::{Mode, ModeGroup, Options},
    dom::{self, DomContent, DomRoot, DomView, Getter, Link, Setter},
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsValue;
use web_sys::HtmlElement;

/// Element tag of the standalone preview/source control.
pub(crate) const TAG: &str = "preview-code-toggle";
/// The selection a mounted group reports to, replaced when another controller updates it.
type Selection = Rc<RefCell<Rc<Cell<ViewMode>>>>;
/// Getter and setter callables of one element property, kept as long as its element's state.
type Accessors = (Getter, Setter);
/// One mounted control group and the state its callbacks read.
struct Mounted {
    /// Group node holding the two buttons.
    group: HtmlElement,
    /// Selection the buttons report to.
    selection: Selection,
    /// Shared native button and event owner.
    controls: RefCell<ModeGroup>,
}
impl Mounted {
    /// Mount the shared owner with the preview/source controller and event spelling.
    fn build(
        view: &DomView,
        selection: &Rc<Cell<ViewMode>>,
        host: Option<&HtmlElement>,
    ) -> Result<Self, ToolUiError> {
        let selection = Rc::new(RefCell::new(Rc::clone(selection)));
        let controls = ModeGroup::new(view, options(view, &selection)?, host)?;
        Ok(Self {
            group: controls.node.clone(),
            selection,
            controls: RefCell::new(controls),
        })
    }
    /// Show the controller's current choice without publishing an event.
    fn paint(&self) {
        self.controls.borrow().paint();
    }
    /// Rebind the controller and localized titles, preserving native button identities.
    fn show(&self, selection: &Rc<Cell<ViewMode>>, view: &DomView) -> Result<(), ToolUiError> {
        let options = options(view, &self.selection)?;
        *self.selection.borrow_mut() = Rc::clone(selection);
        self.controls.borrow_mut().show(view, options)
    }
}
/// Project the existing preview controller into the shared native button owner.
fn options(view: &DomView, selection: &Selection) -> Result<Options, ToolUiError> {
    let modes = [("Eye", "Preview"), ("Code", "Code")]
        .into_iter()
        .map(|(icon, key)| {
            Ok(Mode {
                label: view.i18n.text(key).map_err(ToolUiError::I18n)?,
                icon: Some(icon),
            })
        })
        .collect::<Result<Vec<_>, ToolUiError>>()?;
    let (read, write) = (Rc::clone(selection), Rc::clone(selection));
    Ok(Options {
        modes,
        read: Rc::new(move || {
            if read.borrow().get() == ViewMode::Preview {
                0.
            } else {
                1.
            }
        }),
        write: Rc::new(move |index| {
            write.borrow().set(if index == 0. {
                ViewMode::Preview
            } else {
                ViewMode::Code
            });
        }),
        detail: |index, _| {
            if index == 0 {
                "preview".into()
            } else {
                "code".into()
            }
        },
    })
}

impl DomContent for PreviewCodeToggle {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let mounted = Rc::new(Mounted::build(view, &self.selection, None)?);
        Ok(DomRoot {
            node: mounted.group.clone().into(),
            owner: Some(Box::new(mounted)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
            .ok_or_else(missing_group)?;
        mounted.show(&self.selection, view)
    }
}
/// Report an update of a root this control does not own.
fn missing_group() -> ToolUiError {
    ToolUiError::Host(AttachmentHostError::Presentation(
        "Missing preview/source group".into(),
    ))
}
/// One page-defined preview/source control.
struct ToggleElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// Selected mode, kept across reconnection.
    selection: Rc<Cell<ViewMode>>,
    /// Group mounted while connected, painted by the `mode` setter.
    mounted: Rc<RefCell<Option<Rc<Mounted>>>>,
    /// Accessors of the unreflected `mode` property.
    _mode: Accessors,
}
impl custom_element::CustomElement for ToggleElement {
    fn connected_callback(&mut self) {
        let Some(root) = dom::connected(&self.root) else {
            return;
        };
        // Lifecycle callbacks have no error channel; a failed build leaves the element empty.
        let Ok(mounted) = Mounted::build(&self.view, &self.selection, Some(&root)) else {
            return;
        };
        let _ = root.append_child(&mounted.group);
        *self.mounted.borrow_mut() = Some(Rc::new(mounted));
    }
    fn disconnected_callback(&mut self) {
        if let Some(mounted) = self.mounted.borrow_mut().take() {
            mounted.group.remove();
        }
    }
}
/// Install the unreflected `mode` property reading and writing `selection`.
fn property(
    root: &HtmlElement,
    selection: &Rc<Cell<ViewMode>>,
    mounted: &Rc<RefCell<Option<Rc<Mounted>>>>,
) -> Accessors {
    let read = Rc::clone(selection);
    let get = Getter::new(move || {
        match read.get() {
            ViewMode::Preview => "preview",
            ViewMode::Code => "code",
        }
        .into()
    });
    let (write, group) = (Rc::clone(selection), Rc::clone(mounted));
    let set = Setter::new(move |value: JsValue| {
        // Only the declared modes are operands; other assignments leave the selection alone.
        let mode = match value.as_string().as_deref() {
            Some("preview") => ViewMode::Preview,
            Some("code") => ViewMode::Code,
            _ => return,
        };
        if write.replace(mode) != mode
            && let Some(mounted) = group.borrow().as_ref()
        {
            mounted.paint();
        }
    });
    // A valid property name on a live element only fails when it already exists unconfigurably.
    dom::install(root, "mode", &get, &set);
    (get, set)
}
/// Define the preview/source control once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    dom::define_element(TAG, Vec::new(), &[], move |root, _| {
        let selection = Rc::new(Cell::new(ViewMode::Preview));
        let mounted = Rc::default();
        let mode = property(&root, &selection, &mounted);
        ToggleElement {
            root: js_sys::WeakRef::new(&root),
            view: Rc::clone(&view),
            selection,
            mounted,
            _mode: mode,
        }
    })
}
