//! Repaint observations carried through the existing native reading registrations.
use std::collections::HashSet;

/// Native work since the aggregate's previous canvas frame.
#[derive(Clone, Default)]
pub(crate) struct ReadingChanges {
    /// A native viewport scroll, distinct from a content resize.
    pub(crate) scrolled: bool,
    /// Canvas geometry changed independently of content.
    pub(crate) canvas_resized: bool,
    /// Positional rows whose native content changed size.
    pub(crate) slots: HashSet<egui::Id>,
}
/// Keep observations with the existing repaint owner, without a new registry or UI backedge.
fn key(id: egui::Id) -> egui::Id {
    id.with("native reading changes")
}
/// Consume only this containing viewport's observations.
pub(crate) fn take(context: &egui::Context, id: egui::Id) -> ReadingChanges {
    context
        .data_mut(|data| data.remove_temp::<ReadingChanges>(key(id)))
        .unwrap_or_default()
}
/// Record a native callback without retaining a context-data guard across repaint.
pub(super) fn record(
    context: &egui::Context,
    id: egui::Id,
    update: impl FnOnce(&mut ReadingChanges),
) {
    context.data_mut(|data| {
        let changes = data.get_temp_mut_or_default::<ReadingChanges>(key(id));
        update(changes);
    });
    context.request_repaint();
}

use super::{AttachmentHostError, bounds, host_error};
use crate::browser::event_subscription::EventSubscription;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, HtmlCanvasElement, ResizeObserver};

/// Existing observer and scroll registrations, carrying optional containing-row invalidation.
pub(super) struct Notifications {
    /// Native resize registration.
    observer: ResizeObserver,
    /// Callback kept alive for the observer registration.
    _resize: Closure<dyn FnMut()>,
    /// Independently revoked native scrolling registration.
    _scroll: EventSubscription<Closure<dyn FnMut()>>,
    /// Successfully published dimensions, excluding our own layout changes from resize work.
    measured: Rc<RefCell<Option<[f64; 4]>>>,
}
impl Notifications {
    /// Install the same native registrations with no session or registry backedge.
    pub(super) fn new(
        root: &Element,
        canvas: &HtmlCanvasElement,
        repaint: Rc<RefCell<egui::Context>>,
        (id, owner): (egui::Id, (egui::Id, Option<egui::Id>)),
    ) -> Result<Self, AttachmentHostError> {
        let target = Rc::clone(&repaint);
        let callback = Closure::wrap(Box::new(move || {
            let repaint = target.borrow().clone();
            record(&repaint, id, |changes| changes.scrolled = true);
        }) as Box<dyn FnMut()>);
        let scroll = EventSubscription::new(
            root.parent_element()
                .ok_or_else(|| AttachmentHostError::Host("Missing reading viewport".into()))?
                .into(),
            "scroll",
            callback,
        )
        .map_err(|e| host_error(&e))?;
        let measured = Rc::new(RefCell::new(None));
        let previous = Rc::clone(&measured);
        let element = root.clone();
        let drawing = canvas.clone();
        let callback = Closure::wrap(Box::new(move || {
            let repaint = repaint.borrow().clone();
            resized(&element, &drawing, &previous, &repaint, owner);
        }) as Box<dyn FnMut()>);
        let observer =
            ResizeObserver::new(callback.as_ref().unchecked_ref()).map_err(|e| host_error(&e))?;
        observer.observe(root);
        observer.observe(canvas);
        Ok(Self {
            observer,
            _resize: callback,
            _scroll: scroll,
            measured,
        })
    }
    /// Retain final geometry after successful publication, never before a fallible operation.
    pub(super) fn published(&self, root: &Element, canvas: &HtmlCanvasElement) {
        *self.measured.borrow_mut() = dimensions(root, canvas);
    }
}
impl Drop for Notifications {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}
/// Deliver only externally changed native dimensions to their containing positional owner.
fn resized(
    root: &Element,
    canvas: &HtmlCanvasElement,
    previous: &RefCell<Option<[f64; 4]>>,
    repaint: &egui::Context,
    owner: (egui::Id, Option<egui::Id>),
) {
    let dimensions = dimensions(root, canvas);
    let old = previous.replace(dimensions);
    if dimensions == old {
        return;
    }
    record(repaint, owner.0, |changes| {
        changes.slots.extend(owner.1);
        changes.canvas_resized |= old
            .zip(dimensions)
            .is_some_and(|(old, new)| old[2..] != new[2..]);
    });
}
/// Read final native bounds without borrowing the surface registry.
fn dimensions(root: &Element, canvas: &HtmlCanvasElement) -> Option<[f64; 4]> {
    let root = bounds(root).ok()?;
    let canvas = bounds(canvas).ok()?;
    Some([root.width(), root.height(), canvas.width(), canvas.height()])
}
