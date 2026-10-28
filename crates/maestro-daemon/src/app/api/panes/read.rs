//! `pane.read`: reads a pane's screen through the shared read service.

use super::super::{reads::ReadRequest, responses::pane_not_found};
use crate::app::App;
use maestro_protocol::schema::{ErrorBody, PaneReadParams, ResponseResult};

impl<C: FnMut() -> u128> App<C> {
    /// Reads the named pane's screen.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone or has no process.
    pub(super) fn handle_pane_read(
        &self,
        params: &PaneReadParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(&params.pane_id)?;
        let request = ReadRequest {
            source: params.source,
            format: params.format,
            lines: params.lines,
        };
        self.read_pane(workspace, pane, request)
            .map(|read| ResponseResult::PaneRead { read })
            .ok_or_else(|| pane_not_found(&params.pane_id))
    }
}
