//! Live terminal observations over topology-owned tabs.

use crate::terminal::{model::TerminalState, owners::TerminalRuntimeRegistry};
use maestro_workspace::{tab::Tab, terminal_id::TerminalId, tiling::PaneId};
use std::{collections::HashMap, path::PathBuf};

/// Reads a pane's native directory before its retained terminal metadata.
#[must_use]
pub fn cwd_for_pane<S: std::hash::BuildHasher>(
    tab: &Tab,
    pane: PaneId,
    terminals: &HashMap<TerminalId, TerminalState, S>,
    runtimes: &TerminalRuntimeRegistry,
) -> Option<PathBuf> {
    let terminal = tab.terminal_id(pane)?;
    runtimes
        .get(terminal)
        .and_then(crate::panes::lifecycle::PaneRuntime::cwd)
        .or_else(|| terminals.get(terminal).map(|terminal| terminal.cwd.clone()))
}
