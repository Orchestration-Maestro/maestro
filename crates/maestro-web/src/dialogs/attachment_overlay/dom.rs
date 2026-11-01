//! Native attachment chrome, registration and element-owned lifetime.
use super::AttachmentOverlay;
use crate::{
    SharedAttachment,
    browser::{
        attachment_content::browser::append, event_subscription::EventSubscription, timer::Timer,
    },
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        mode_toggle::ModeGroup,
    },
    dom::{self, DomContent, DomRoot, DomView, Link},
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Event, HtmlElement};
mod content;
mod inputs;
mod update;
/// One owned native listener.
type EventBinding = EventSubscription<Closure<dyn Fn(Event) -> Result<(), JsValue>>>;
thread_local! {
    /// Operand consumed by the synchronous native constructor.
    static CREATING: RefCell<Option<Rc<State>>> = const { RefCell::new(None) };
}
/// Element-owned state; every route back to its element is weak.
pub(super) struct State {
    /// Page operands of the creating caller, not the first registered caller.
    view: Rc<DomView>,
    /// Original mutable application record.
    attachment: SharedAttachment,
    /// Non-owning native root.
    root: RefCell<Option<Link>>,
    /// Close callback taken before notification.
    on_close: RefCell<Option<Rc<dyn Fn()>>>,
    /// Admission closes before callbacks run.
    closed: Cell<bool>,
    /// Current text/file choice.
    extracted: Rc<Cell<bool>>,
    /// Native listeners retired on disconnect.
    events: RefCell<Vec<EventBinding>>,
    /// Shared native mode control.
    modes: RefCell<Option<ModeGroup>>,
    /// Document owners and pending page delivery.
    body: RefCell<Option<content::Body>>,
    /// Pending page delivery, cancelled on disconnect or mode change.
    frame: RefCell<Option<Timer>>,
    /// Last published body operands, compared before resetting document resources.
    previous: RefCell<Option<inputs::Inputs>>,
    /// File failures stay visible until a deliberate mode change.
    failed: Cell<bool>,
}
impl AttachmentOverlay {
    /// Open an element-owned native preview in the supplied page.
    /// # Errors
    /// Returns creation, append, localization or native registration failure synchronously.
    pub fn open(
        view: &Rc<DomView>,
        attachment: SharedAttachment,
        on_close: Option<Rc<dyn Fn()>>,
    ) -> Result<Self, ToolUiError> {
        define(view)?;
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let state = Rc::new(State {
            view: Rc::clone(view),
            attachment,
            root: RefCell::default(),
            on_close: RefCell::new(on_close),
            closed: Cell::new(false),
            extracted: Rc::default(),
            events: RefCell::default(),
            modes: RefCell::default(),
            body: RefCell::default(),
            frame: RefCell::default(),
            previous: RefCell::default(),
            failed: Cell::new(false),
        });
        let previous = CREATING.replace(Some(Rc::clone(&state)));
        let created = view.document.create_element("attachment-overlay");
        CREATING.replace(previous);
        let root: HtmlElement = created
            .map_err(|error| ToolUiError::native(&error))?
            .dyn_into()
            .map_err(|error| ToolUiError::native(&error))?;
        *state.root.borrow_mut() = Some(js_sys::WeakRef::new(&root));
        if let Err(error) = state.build(&root) {
            state.disconnect();
            return Err(error);
        }
        let body = view.document.body().ok_or_else(|| {
            state.disconnect();
            ToolUiError::Presentation("Missing document body".into())
        })?;
        if let Err(error) = body.append_child(&root) {
            state.disconnect();
            return Err(ToolUiError::native(&error));
        }
        if let Err(error) = state.listen_keys() {
            root.remove();
            return Err(error);
        }
        Ok(Self { state })
    }
}
impl State {
    /// Build fixed chrome before publishing the element into the page.
    fn build(self: &Rc<Self>, root: &HtmlElement) -> Result<(), ToolUiError> {
        let backdrop = self.element("div", "maestro-attachment-overlay")?;
        let header = self.element("header", "maestro-attachment-header")?;
        let name = self.element("span", "maestro-attachment-name")?;
        let filename = self
            .attachment
            .try_read()
            .map_err(|_| ToolUiError::Presentation("Attachment borrow conflict".into()))?
            .file_name
            .clone();
        name.set_text_content(Some(&filename));
        append(&header, &name)?;
        let actions = self.element("div", "maestro-attachment-actions")?;
        let modes = ModeGroup::new(&self.view, self.mode_options()?, None)?;
        self.bind(modes.node.clone().into(), "mode-change", Action::Mode)?;
        append(&actions, &modes.node)?;
        *self.modes.borrow_mut() = Some(modes);
        for (icon, attribute, label) in [
            ("Download", "data-download", "Download"),
            ("X", "data-close", "Close"),
        ] {
            let control = self.control(icon, &self.view.i18n.text(label)?)?;
            control
                .set_attribute(attribute, "")
                .map_err(|error| ToolUiError::native(&error))?;
            self.bind(
                control.clone().into(),
                "click",
                if icon == "X" {
                    Action::Close
                } else {
                    Action::Download
                },
            )?;
            append(&actions, &control)?;
        }
        append(&header, &actions)?;
        let body = self.element("div", "maestro-attachment-body")?;
        self.bind(header.clone().into(), "click", Action::Inside)?;
        self.bind(body.clone().into(), "click", Action::Inside)?;
        self.bind(backdrop.clone().into(), "click", Action::Close)?;
        append(&backdrop, &header)?;
        append(&backdrop, &body)?;
        append(root, &backdrop)?;
        self.show_content()
    }
    /// Create a native element in the retained document.
    fn element(&self, tag: &str, class: &str) -> Result<HtmlElement, ToolUiError> {
        crate::browser::attachment_content::browser::element(&self.view.document, tag, class)?
            .dyn_into()
            .map_err(|error| ToolUiError::native(&error))
    }
    /// Reuse the shared ghost button and glyph owners; the localized title names the glyph-only control.
    fn control(&self, icon: &str, title: &str) -> Result<HtmlElement, ToolUiError> {
        let glyph = crate::icons::icon_node(icon)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?
            .ok_or_else(|| ToolUiError::Presentation("Missing icon".into()))?;
        let node = crate::icons::icon_dom(&glyph, crate::icons::IconSize::Sm, None)
            .map_err(|error| ToolUiError::native(&error))?;
        let control = button(
            &self.view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Icon,
                title: Some(title),
                ..ButtonProps::default()
            },
            &[node.into()],
        )
        .map_err(|error| ToolUiError::native(&error))?;
        Ok(control.into())
    }
    /// Admit the key handler only after body publication succeeds.
    fn listen_keys(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let window = self
            .view
            .document
            .default_view()
            .ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
        self.bind(window.into(), "keydown", Action::Key)
    }
    /// Bind a weak state route through the shared subscription owner.
    fn bind(
        self: &Rc<Self>,
        target: web_sys::EventTarget,
        name: &'static str,
        action: Action,
    ) -> Result<(), ToolUiError> {
        let weak = Rc::downgrade(self);
        let callback = Closure::new(move |event: Event| {
            if let Some(state) = weak.upgrade().filter(|state| !state.closed.get()) {
                state
                    .handle(&event, action)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
            }
            Ok(())
        });
        let binding = EventSubscription::new(target, name, callback)
            .map_err(|error| ToolUiError::native(&error))?;
        self.events.borrow_mut().push(binding);
        Ok(())
    }
    /// Deliver one native action with no retained operand borrow.
    fn handle(self: &Rc<Self>, event: &Event, action: Action) -> Result<(), ToolUiError> {
        match action {
            Action::Inside => event.stop_propagation(),
            Action::Close => self.close(),
            Action::Key => {
                if event
                    .dyn_ref::<web_sys::KeyboardEvent>()
                    .is_some_and(|event| event.key() == "Escape")
                {
                    self.close();
                }
            }
            Action::Download => {
                crate::utils::download::download(&self.view.document, &self.attachment)?;
            }
            Action::Mode => {
                event.stop_propagation();
                self.show_content()?;
            }
        }
        Ok(())
    }
    /// Stop document work before notifying, retaining the callback for exactly one close.
    fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        self.disconnect();
        self.extracted.set(false);
        self.failed.set(false);
        let callback = self.on_close.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
        let root = self.root.borrow().as_ref().and_then(dom::live);
        if let Some(root) = root {
            root.remove();
        }
    }
    /// Revoke pending work and listeners without invoking caller code under a borrow.
    fn disconnect(&self) {
        let frame = self.frame.borrow_mut().take();
        let body = self.body.borrow_mut().take();
        let events = self.events.take();
        let modes = self.modes.borrow_mut().take();
        let previous = self.previous.borrow_mut().take();
        drop((frame, body, events, modes, previous));
    }
}
/// Native action selected when binding a control.
#[derive(Clone, Copy)]
enum Action {
    /// Stop bubbling inside chrome or content.
    Inside,
    /// Close via backdrop or control.
    Close,
    /// Close on Escape only.
    Key,
    /// Original-byte download.
    Download,
    /// Refresh after the shared mode owner changes selection.
    Mode,
}
/// Attachment element retains its creating operands even after its caller drops the returned handle.
struct AttachmentElement {
    /// State exists only for an explicitly opened element.
    state: Option<Rc<State>>,
}
impl custom_element::CustomElement for AttachmentElement {
    fn disconnected_callback(&mut self) {
        if let Some(state) = &self.state {
            state.disconnect();
        }
    }
}
/// Register once without retaining the first opening's operands in the constructor.
fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let window = view
        .document
        .default_view()
        .ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
    if window
        .custom_elements()
        .get("attachment-overlay")
        .is_undefined()
    {
        dom::define_element("attachment-overlay", Vec::new(), &[], |_, _| {
            AttachmentElement {
                state: CREATING.take(),
            }
        })
        .map_err(|error| ToolUiError::native(&error))?;
    }
    Ok(())
}
impl DomContent for AttachmentOverlay {
    fn mount(&self, _: &DomView) -> Result<DomRoot, ToolUiError> {
        let state = &self.state;
        let root = state
            .root
            .borrow()
            .as_ref()
            .and_then(dom::live)
            .ok_or_else(|| ToolUiError::Presentation("Missing overlay root".into()))?;
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(Rc::clone(state))),
        })
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        self.state.refresh()
    }
}
