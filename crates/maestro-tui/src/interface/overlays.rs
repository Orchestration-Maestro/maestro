//! Retained overlay identities and their controls.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

use super::{ComponentHandle, OverlayHandle, OverlayOptions, TUI};

/// A component target with immutable fallback data.
#[derive(Clone)]
pub(super) enum Focus {
    /// Explicit external focus, possibly absent.
    Base(Option<ComponentHandle>),
    /// An overlay-derived component and its captured fallback.
    Captured(Rc<Capture>),
}

impl Default for Focus {
    fn default() -> Self {
        Self::Base(None)
    }
}

impl Focus {
    /// Borrows the input recipient without copying its identity.
    pub(super) fn component(&self) -> Option<&ComponentHandle> {
        match self {
            Self::Base(component) => component.as_ref(),
            Self::Captured(capture) => Some(&capture.component),
        }
    }
}

/// The component and focus value retained at creation.
pub(super) struct Capture {
    /// Rendered component and potential input recipient.
    pub component: ComponentHandle,
    /// Focus captured before this node existed.
    pre_focus: Focus,
}

/// One identity retained by the owner and its control handle.
pub(super) struct Entry {
    /// Immutable component and captured fallback.
    pub capture: Rc<Capture>,
    /// Options shared with the caller.
    pub options: Rc<RefCell<OverlayOptions>>,
    /// Temporary hiding, independent of callback visibility.
    hidden: Cell<bool>,
    /// Visual order, independent of stack order.
    pub order: Cell<usize>,
}

/// Controls one entry while keeping its owner alive.
struct Handle {
    /// Owner shared with every writer clone.
    tui: TUI,
    /// The controlled identity, retained even after removal.
    entry: Rc<Entry>,
}

impl Entry {
    /// Whether showing this entry automatically captures focus.
    pub(super) fn capturing(&self) -> bool {
        self.options.borrow().non_capturing != Some(true)
    }
}

impl TUI {
    /// Shows `component` with caller-editable options. See the [overlay lifecycle](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/terminal/overlays.md).
    ///
    /// # Errors
    /// Returns cursor-hiding errors after completed stack and conditional focus changes; no frame is
    /// requested on that failure.
    pub fn show_overlay(
        &self,
        component: ComponentHandle,
        options: Option<Rc<RefCell<OverlayOptions>>>,
    ) -> io::Result<Box<dyn OverlayHandle>> {
        let entry = Rc::new(Entry {
            capture: Rc::new(Capture {
                component,
                pre_focus: self.shared.input.borrow().focused.clone(),
            }),
            options: options.unwrap_or_default(),
            hidden: Cell::new(false),
            order: Cell::new(self.next_focus_order()),
        });
        self.shared.overlays.borrow_mut().push(Rc::clone(&entry));
        if entry.capturing() && self.focus_available(&entry) {
            self.commit_focus(Focus::Captured(Rc::clone(&entry.capture)));
        }
        self.shared.terminal.borrow_mut().hide_cursor()?;
        self.request_render(false);
        Ok(Box::new(Handle {
            tui: self.clone(),
            entry,
        }))
    }

    /// Removes the last-created overlay; an empty stack does nothing.
    ///
    /// # Errors
    /// Returns cursor-hiding errors after removal and any applicable focus restoration, without
    /// requesting a frame on that failure.
    pub fn hide_overlay(&self) -> io::Result<()> {
        let entry = self.shared.overlays.borrow().last().cloned();
        if let Some(entry) = entry {
            self.remove_overlay(&entry)?;
        }
        Ok(())
    }

    /// Whether the visibility walk observes a visible entry, including noncapturing entries.
    /// A callback may remove the observed entry before this method returns.
    #[must_use]
    pub fn has_overlay(&self) -> bool {
        let length = self.shared.overlays.borrow().len();
        (0..length).any(|index| {
            self.overlay_at(index)
                .is_some_and(|entry| self.overlay_visible(&entry))
        })
    }

    /// The entry currently at `index`, retaining it without a stack borrow.
    pub(super) fn overlay_at(&self, index: usize) -> Option<Rc<Entry>> {
        self.shared.overlays.borrow().get(index).cloned()
    }

    /// Whether the entry is still in the stack.
    fn attached(&self, entry: &Rc<Entry>) -> bool {
        self.shared
            .overlays
            .borrow()
            .iter()
            .any(|item| Rc::ptr_eq(item, entry))
    }

    /// Visibility at current terminal dimensions; hidden entries skip the callback.
    pub(super) fn overlay_visible(&self, entry: &Entry) -> bool {
        if entry.hidden.get() {
            return false;
        }
        let visible = entry.options.borrow().visible.clone();
        visible.is_none_or(|visible| {
            let (width, height) = {
                let terminal = self.shared.terminal.borrow();
                (terminal.columns(), terminal.rows())
            };
            visible(width, height)
        })
    }

    /// Checks the retained component after its source visibility observation.
    fn focus_available(&self, entry: &Rc<Entry>) -> bool {
        self.attached(entry)
            && self.overlay_visible(entry)
            && self.component_available(&entry.capture.component, Some(entry))
    }

    /// Accepts a retained observation or samples a finite set of component aliases.
    fn component_available(
        &self,
        component: &ComponentHandle,
        accepted: Option<&Rc<Entry>>,
    ) -> bool {
        if accepted.is_some_and(|entry| self.attached(entry) && !entry.hidden.get()) {
            return true;
        }
        let aliases: Vec<_> = self
            .shared
            .overlays
            .borrow()
            .iter()
            .filter(|entry| Rc::ptr_eq(&entry.capture.component, component))
            .cloned()
            .collect();
        aliases.into_iter().any(|entry| {
            self.attached(&entry)
                && !entry.hidden.get()
                && self.overlay_visible(&entry)
                && self.attached(&entry)
                && !entry.hidden.get()
        })
    }

    /// Rereads the first accepted numeric slot once, including a vacated slot.
    fn top_capturing(&self) -> Option<(Rc<Entry>, bool)> {
        let length = self.shared.overlays.borrow().len();
        for index in (0..length).rev() {
            let Some(candidate) = self.overlay_at(index) else {
                continue;
            };
            if !candidate.capturing() || !self.overlay_visible(&candidate) {
                continue;
            }
            let selected = self.overlay_at(index)?;
            let accepted = Rc::ptr_eq(&candidate, &selected);
            return Some((selected, accepted));
        }
        None
    }

    /// The next visual order.
    fn next_focus_order(&self) -> usize {
        let order = self.shared.focus_order.get().saturating_add(1);
        self.shared.focus_order.set(order);
        order
    }

    /// Whether this component currently owns focus.
    fn overlay_focused(&self, entry: &Entry) -> bool {
        self.shared
            .input
            .borrow()
            .focused
            .component()
            .is_some_and(|focus| Rc::ptr_eq(focus, &entry.capture.component))
    }

    /// Resolves a component capture, consuming older nodes when unavailable.
    fn resolve_focus(&self, mut focus: Focus, accepted: Option<&Rc<Entry>>) {
        let mut observation = accepted;
        while let Focus::Captured(capture) = &focus {
            if self.component_available(&capture.component, observation) {
                break;
            }
            focus = capture.pre_focus.clone();
            observation = None;
        }
        self.commit_focus(focus);
    }

    /// Selects a top candidate or captured fallback; only unfocus excludes itself.
    fn restore_overlay_focus(&self, entry: &Rc<Entry>, unfocus: bool) {
        if let Some((top, accepted)) = self
            .top_capturing()
            .filter(|(top, _)| !unfocus || !Rc::ptr_eq(top, entry))
        {
            self.resolve_focus(
                Focus::Captured(Rc::clone(&top.capture)),
                accepted.then_some(&top),
            );
        } else {
            self.resolve_focus(entry.capture.pre_focus.clone(), None);
        }
    }

    /// Removes one identity, completing state changes before terminal effects.
    fn remove_overlay(&self, entry: &Rc<Entry>) -> io::Result<()> {
        let removed = {
            let mut stack = self.shared.overlays.borrow_mut();
            stack
                .iter()
                .position(|item| Rc::ptr_eq(item, entry))
                .map(|index| stack.remove(index))
        };
        if removed.is_none() {
            return Ok(());
        }
        if self.overlay_focused(entry) {
            self.restore_overlay_focus(entry, false);
        }
        if self.shared.overlays.borrow().is_empty() {
            self.shared.terminal.borrow_mut().hide_cursor()?;
        }
        self.request_render(false);
        Ok(())
    }

    /// Repairs focus just before ordinary input dispatch.
    pub(super) fn repair_overlay_focus(&self) {
        let entry = {
            let input = self.shared.input.borrow();
            self.shared
                .overlays
                .borrow()
                .iter()
                .find(|entry| {
                    input
                        .focused
                        .component()
                        .is_some_and(|focus| Rc::ptr_eq(focus, &entry.capture.component))
                })
                .cloned()
        };
        if let Some(entry) = entry
            && !self.overlay_visible(&entry)
        {
            self.restore_overlay_focus(&entry, false);
        }
    }
}

impl OverlayHandle for Handle {
    fn hide(&mut self) -> io::Result<()> {
        self.tui.remove_overlay(&self.entry)
    }

    fn set_hidden(&mut self, hidden: bool) {
        if !self.tui.attached(&self.entry) || self.entry.hidden.replace(hidden) == hidden {
            return;
        }
        if hidden {
            if self.tui.overlay_focused(&self.entry) {
                self.tui.restore_overlay_focus(&self.entry, false);
            }
        } else if self.entry.capturing() && self.tui.focus_available(&self.entry) {
            self.entry.order.set(self.tui.next_focus_order());
            self.tui
                .commit_focus(Focus::Captured(Rc::clone(&self.entry.capture)));
        }
        self.tui.request_render(false);
    }

    fn is_hidden(&self) -> bool {
        self.entry.hidden.get()
    }

    fn focus(&mut self) {
        if !self.tui.focus_available(&self.entry) {
            return;
        }
        if !self.is_focused() {
            self.tui
                .commit_focus(Focus::Captured(Rc::clone(&self.entry.capture)));
        }
        self.entry.order.set(self.tui.next_focus_order());
        self.tui.request_render(false);
    }

    fn unfocus(&mut self) {
        if !self.tui.attached(&self.entry) || !self.is_focused() {
            return;
        }
        self.tui.restore_overlay_focus(&self.entry, true);
        self.tui.request_render(false);
    }

    fn is_focused(&self) -> bool {
        self.tui.overlay_focused(&self.entry)
    }
}
