//! The one progress indicator the status area shows, and the idle rows that keep its height
//! after the indicator ends.

use super::InteractiveMode;
use crate::CountdownTimer;
use maestro_tui::{Component, Loader};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// What a status indicator reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatusKind {
    /// A response is running.
    Working,
    /// A failed request waits for its next attempt.
    Retry,
    /// The context is being compacted.
    Compaction,
    /// A branch is being summarized.
    BranchSummary,
}

/// A shown indicator: its loader and, for a retry, the countdown that updates it.
struct Indicator {
    /// What the indicator reports.
    kind: StatusKind,
    /// The spinner row.
    loader: Loader,
    /// The retry countdown, stopped with the indicator.
    countdown: Option<CountdownTimer>,
}

impl Indicator {
    /// Stops the countdown frames and the spinner.
    fn dispose(self) {
        if let Some(countdown) = self.countdown {
            countdown.dispose();
        }
        self.loader.stop();
    }
}

/// The status area's shown indicator and whether its idle rows stand in for one.
#[derive(Default)]
pub(super) struct StatusArea {
    /// The shown indicator.
    active: RefCell<Option<Indicator>>,
    /// Whether the idle rows are shown.
    idle: Cell<bool>,
}

/// Two rows of spaces, as tall as an indicator.
struct IdleStatus;

impl Component for IdleStatus {
    fn render(&self, width: usize) -> Vec<String> {
        vec![" ".repeat(width); 2]
    }
}

impl InteractiveMode {
    /// Shows `loader` as the indicator of `kind`, with the countdown that updates it, in place
    /// of the indicator or idle rows shown before; the replaced indicator stops.
    pub(super) fn show_status_indicator(
        &self,
        kind: StatusKind,
        loader: Loader,
        countdown: Option<CountdownTimer>,
    ) {
        self.status_container.clear();
        self.status_container.add_child(Rc::new(loader.clone()));
        self.progress.status.idle.set(false);
        let replaced = self.progress.status.active.replace(Some(Indicator {
            kind,
            loader,
            countdown,
        }));
        if let Some(replaced) = replaced {
            replaced.dispose();
        }
    }

    /// Stops the shown indicator when it reports `kind`, or any indicator when `kind` is
    /// `None`. With clear-on-shrink on, an area that showed an indicator or the idle rows
    /// shows the idle rows; otherwise it is empty.
    pub(super) fn clear_status_indicator(&self, kind: Option<StatusKind>) {
        if kind.is_some() && self.status_kind() != kind {
            return;
        }
        let active = self.progress.status.active.take();
        let shown = active.is_some() || self.progress.status.idle.get();
        if let Some(active) = active {
            active.dispose();
        }
        self.status_container.clear();
        let idle = shown && self.ui.get_clear_on_shrink();
        if idle {
            self.status_container.add_child(Rc::new(IdleStatus));
        }
        self.progress.status.idle.set(idle);
    }

    /// What the shown indicator reports.
    pub(super) fn status_kind(&self) -> Option<StatusKind> {
        self.progress
            .status
            .active
            .borrow()
            .as_ref()
            .map(|indicator| indicator.kind)
    }

    /// The loader of the shown indicator when it reports `kind`.
    pub(super) fn status_loader(&self, kind: StatusKind) -> Option<Loader> {
        self.progress
            .status
            .active
            .borrow()
            .as_ref()
            .filter(|indicator| indicator.kind == kind)
            .map(|indicator| indicator.loader.clone())
    }

    /// Applies the clear-on-shrink setting; turning it off removes the idle rows while no
    /// indicator is shown.
    pub(super) fn apply_clear_on_shrink(&self, enabled: bool) {
        self.ui.set_clear_on_shrink(enabled);
        if !enabled && self.progress.status.active.borrow().is_none() {
            self.status_container.clear();
            self.progress.status.idle.set(false);
        }
    }
}
