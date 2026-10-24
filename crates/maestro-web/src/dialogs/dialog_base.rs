//! Owned dialog lifetimes in one canvas context.
use super::surface::{DialogBody, DialogProps};
use crate::dialogs::surface::DialogUiError;
use crate::{
    I18n,
    browser::attachment_content::{AttachmentHost, FocusTarget, identity},
};
use maestro_theme::BrandPresentation;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
/// One drawing context with weak dialog registrations.
#[derive(Clone)]
pub struct DialogContext {
    /// Shared platform and weak owner registry.
    inner: Rc<Context>,
}
/// Context-owned platform effects and non-owning paint handles.
struct Context {
    /// Canvas drawing owner.
    drawing: egui::Context,
    /// Replaceable native content owner.
    host: Rc<dyn AttachmentHost>,
    /// Independent dialogs in opening order.
    owners: RefCell<Vec<Weak<State>>>,
    /// Pending next-frame effects with their completion flags.
    restorations: RefCell<Vec<(crate::browser::timer::Timer, Rc<Cell<bool>>)>>,
}
/// Mutable caller operands, owned only by the retained handle.
pub(super) struct State {
    /// Platform context.
    pub(super) context: DialogContext,
    /// Independent modal identity.
    pub(super) id: egui::Id,
    /// Live props, snapshotted before calling user code.
    pub(super) props: RefCell<DialogProps>,
    /// Focus captured before admission, consumed at removal.
    focus: RefCell<Option<Rc<FocusTarget>>>,
    /// Last admitted pointer gesture, independent of stale toolkit clicks.
    pub(super) pressed: RefCell<Option<bool>>,
    /// Retained Close texture, updated only when its painted operands change.
    pub(super) ring:
        RefCell<crate::browser::control_paint::Painter<super::surface::close::CloseKey>>,
    /// Mount identity invalidated before removal effects or caller callbacks.
    pub(super) mount: Cell<u64>,
    /// Mounted Close properties, discarded before any removal effects.
    pub(super) transitions: RefCell<Option<super::surface::close::CloseTransitions>>,
    /// Non-owning aliases for the existing reading transaction.
    #[cfg(target_arch = "wasm32")]
    pub(super) native: RefCell<super::surface::browser::Native>,
}
/// Retained shell owner; dropping it releases its handling route.
pub struct DialogBase {
    /// Private lifetime shared only with temporary paint snapshots.
    state: Rc<State>,
}
impl DialogContext {
    /// Retain the canvas and existing native content owner.
    #[must_use]
    pub fn new(context: egui::Context, host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            inner: Rc::new(Context {
                drawing: context,
                host,
                owners: RefCell::default(),
                restorations: RefCell::default(),
            }),
        }
    }
    /// Paint open retained dialogs and dispatch their input.
    /// # Errors
    /// Returns reached localization or platform failures.
    pub fn render<F: ?Sized>(
        &self,
        i18n: &I18n<F>,
        presentation: &BrandPresentation<'_>,
    ) -> Result<(), DialogUiError> {
        let completed: Vec<_> = self
            .inner
            .restorations
            .borrow_mut()
            .extract_if(.., |(_, done)| done.get())
            .collect();
        drop(completed);
        let owners = self.inner.owners.borrow().clone();
        let escape = self.inner.host.take_escape()
            | self
                .inner
                .drawing
                .input(|i| i.key_pressed(egui::Key::Escape));
        let mut result = Ok(());
        for registration in owners {
            let Some(owner) = registration.upgrade() else {
                continue;
            };
            if !owner.props.borrow().is_open {
                continue;
            }
            let current = if escape {
                owner.close(true)
            } else {
                super::surface::draw(&owner, i18n, presentation)
            };
            result = result.and(current);
        }
        result
    }
    /// Shared canvas context for the source-mapped shell painter.
    pub(super) fn drawing(&self) -> &egui::Context {
        &self.inner.drawing
    }
    /// Existing native effect owner.
    pub(super) fn host(&self) -> &dyn AttachmentHost {
        self.inner.host.as_ref()
    }
}
impl DialogBase {
    /// Retain a closed shell with base dimensions.
    #[must_use]
    pub fn new(context: DialogContext, children: DialogBody) -> Self {
        let mut props = DialogProps::new(false, children);
        props.height = "min(600px, 80vh)".into();
        Self {
            state: Rc::new(State {
                context,
                id: identity(),
                props: RefCell::new(props),
                focus: RefCell::default(),
                pressed: RefCell::default(),
                ring: RefCell::default(),
                mount: Cell::new(0),
                transitions: RefCell::default(),
                #[cfg(target_arch = "wasm32")]
                native: RefCell::default(),
            }),
        }
    }
    /// Replace the live caller operands.
    /// # Errors
    /// Returns reached native failures.
    pub fn set_props(&self, props: DialogProps) -> Result<(), DialogUiError> {
        let open = props.is_open;
        let was_open = self.state.props.borrow().is_open;
        let mut props = props;
        props.is_open = was_open;
        let previous = self.state.props.replace(props);
        drop(previous);
        self.state.context.inner.drawing.request_repaint();
        if open { self.open() } else { self.close() }
    }
    /// Eagerly admit this modal before any render or result polling.
    /// # Errors
    /// Returns native admission failures.
    pub fn open(&self) -> Result<(), DialogUiError> {
        if self.state.props.borrow().is_open {
            return Ok(());
        }
        let focus = self.state.context.inner.host.capture_focus()?;
        let target = focus_target(&self.state.context.inner.drawing, focus);
        if let Err(error) = self
            .state
            .context
            .inner
            .host
            .set_overlay(self.state.id, true)
        {
            let _ = self
                .state
                .context
                .inner
                .host
                .set_overlay(self.state.id, false);
            return Err(DialogUiError::Host(error));
        }
        *self.state.focus.borrow_mut() = target;
        self.state.props.borrow_mut().is_open = true;
        self.state
            .context
            .inner
            .owners
            .borrow_mut()
            .push(Rc::downgrade(&self.state));
        self.state.context.inner.drawing.request_repaint();
        Ok(())
    }
    /// Remove this shell before invoking its optional notification.
    /// # Errors
    /// Returns reached platform failures.
    pub fn close(&self) -> Result<(), DialogUiError> {
        self.state.close(true)
    }
}
impl State {
    /// Revoke before removal; caller notification is never run under a borrow.
    pub(super) fn close(&self, notify: bool) -> Result<(), DialogUiError> {
        let callback = {
            let mut props = self.props.borrow_mut();
            if !props.is_open {
                return Ok(());
            }
            props.is_open = false;
            props.on_close.clone()
        };
        self.mount.set(self.mount.get().wrapping_add(1));
        self.transitions.take();
        self.context
            .inner
            .drawing
            .memory_mut(|memory| memory.surrender_focus(self.id.with("close")));
        self.context
            .inner
            .owners
            .borrow_mut()
            .retain(|owner| owner.as_ptr() != self);
        self.context.inner.host.remove(self.id);
        let removed = self.context.inner.host.set_overlay(self.id, false);
        let focus = self.focus.take();
        let restored = focus.map_or(Ok(()), |target| {
            let complete = Rc::new(Cell::new(false));
            let delivered = complete.clone();
            let target: Rc<FocusTarget> = Rc::new(move || {
                delivered.set(true);
                target()
            });
            let timer = self.context.inner.host.restore_focus(target)?;
            self.context
                .inner
                .restorations
                .borrow_mut()
                .push((timer, complete));
            Ok(())
        });
        self.context.inner.drawing.request_repaint();
        if notify && let Some(callback) = callback {
            callback();
        }
        removed.and(restored).map_err(DialogUiError::Host)
    }
}
impl Drop for DialogBase {
    fn drop(&mut self) {
        let _ = self.state.close(false);
    }
}

/// Keep native and canvas focus together without scheduling an absent target.
fn focus_target(
    drawing: &egui::Context,
    native: Option<Rc<FocusTarget>>,
) -> Option<Rc<FocusTarget>> {
    let canvas = drawing.memory(egui::Memory::focused);
    if native.is_none() && canvas.is_none() {
        return None;
    }
    let drawing = drawing.clone();
    Some(Rc::new(move || {
        if let Some(native) = &native {
            native()?;
        }
        if let Some(id) = canvas {
            drawing.memory_mut(|memory| memory.request_focus(id));
        }
        Ok(())
    }))
}

#[cfg(target_arch = "wasm32")]
impl DialogBase {
    /// Set literal panel classes for the next browser render without replacing its body.
    pub fn set_class_name(&self, class_name: String) {
        self.state.native.borrow_mut().class_name = class_name.into();
        self.state.context.drawing().request_repaint();
    }
}
