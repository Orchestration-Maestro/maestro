//! The shared read service: the one conversion from a requested source, format and line count
//! to a pane's screen text, for every command that reads a resolved pane.

use crate::{
    app::App,
    pane::{PaneRuntime, terminal::TerminalReadSnapshot},
};
use maestro_protocol::schema::{PaneReadResult, ReadFormat, ReadSource};
use maestro_workspace::layout::PaneId;

/// Most lines one read may ask for.
const MAX_READ_LINES: u32 = 1000;

/// Lines of recent history a read returns when it names no count.
const DEFAULT_RECENT_LINES: usize = 80;

/// What a read asks of a pane's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReadRequest {
    /// Which rows are read.
    pub(super) source: ReadSource,
    /// Whether styling is kept.
    pub(super) format: ReadFormat,
    /// Most lines to return; absent keeps every line of a viewport read.
    pub(super) lines: Option<u32>,
}

/// Reads `runtime` as `request` asks, clipping the text to the requested lines.
fn read_terminal_snapshot(runtime: &PaneRuntime, request: ReadRequest) -> TerminalReadSnapshot {
    let limit = request
        .lines
        .map(|lines| lines.min(MAX_READ_LINES) as usize);
    let recent = limit.unwrap_or(DEFAULT_RECENT_LINES);
    match (request.format, request.source) {
        (ReadFormat::Text, ReadSource::Visible) => {
            TerminalReadSnapshot::limited(runtime.visible_text(), limit)
        }
        (ReadFormat::Text, ReadSource::Recent) => runtime.recent_text_snapshot(recent),
        (ReadFormat::Text, ReadSource::RecentUnwrapped) => {
            runtime.recent_unwrapped_text_snapshot(recent)
        }
        (_, ReadSource::Detection) => {
            TerminalReadSnapshot::limited(runtime.detection_text(), limit)
        }
        (ReadFormat::Ansi, ReadSource::Visible) => {
            TerminalReadSnapshot::limited(runtime.visible_ansi(), limit)
        }
        (ReadFormat::Ansi, ReadSource::Recent) => runtime.recent_ansi_snapshot(recent),
        (ReadFormat::Ansi, ReadSource::RecentUnwrapped) => {
            runtime.recent_unwrapped_ansi_snapshot(recent)
        }
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Reads the screen of a resolved pane, `None` when the pane has no tab or no process.
    ///
    /// The result names the pane, its workspace and its tab with their public handles.
    pub(super) fn read_pane(
        &self,
        workspace: usize,
        pane: PaneId,
        request: ReadRequest,
    ) -> Option<PaneReadResult> {
        let owner = self.state.topology.workspaces.get(workspace)?;
        let tab = owner.find_tab_index_for_pane(pane)?;
        let snapshot = read_terminal_snapshot(self.pane_runtime(workspace, pane)?, request);
        Some(PaneReadResult {
            pane_id: self.state.public_pane_id(workspace, pane)?,
            workspace_id: owner.id.clone(),
            tab_id: self.state.public_tab_id(workspace, tab)?,
            source: request.source,
            format: request.format,
            text: snapshot.text,
            revision: 0,
            truncated: snapshot.truncated,
        })
    }
}
