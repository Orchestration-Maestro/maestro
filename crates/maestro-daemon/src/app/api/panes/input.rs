//! Pane input: right-click routing and the text, key and combined writes to a pane's process.

use super::super::responses::{api_error, pane_not_found};
use crate::{app::App, input::TerminalKey, key_combo::parse_key_combo, pane::PaneRuntime};
use maestro_protocol::schema::{
    ErrorBody, PaneInputSetParams, PaneRightClickTarget, PaneSendInputParams, PaneSendKeysParams,
    PaneSendTextParams, ResponseResult,
};

/// Reads one key spelling of a request: the legacy control-c spellings and a literal plus,
/// then the shared key-combination grammar. A prefix binding names no key there.
fn api_key(key: &str) -> Option<TerminalKey> {
    let key = key.trim();
    let spelling = match key {
        "C-c" | "c-c" => "ctrl+c",
        "+" => "plus",
        other => other,
    };
    let (code, modifiers) = parse_key_combo(spelling)?;
    Some(TerminalKey::new(code, modifiers))
}

/// Encodes every key for the process's current terminal modes, or refuses the first one
/// that is not a key before anything is encoded for writing.
fn encode_keys(runtime: &PaneRuntime, keys: &[String]) -> Result<Vec<Vec<u8>>, ErrorBody> {
    keys.iter()
        .map(|key| {
            api_key(key)
                .map(|encoded| runtime.encode_terminal_key(encoded))
                .ok_or_else(|| api_error("invalid_key", format!("unsupported key {key}")))
        })
        .collect()
}

/// Writes each chunk to the process, stopping at the first one it cannot take.
fn deliver(
    runtime: &PaneRuntime,
    chunks: impl IntoIterator<Item = Vec<u8>>,
) -> Result<ResponseResult, ErrorBody> {
    for bytes in chunks {
        runtime
            .try_send_bytes(bytes)
            .map_err(|error| api_error("pane_send_failed", error.to_string()))?;
    }
    Ok(ResponseResult::Ok {})
}

impl<C: FnMut() -> u128> App<C> {
    /// Chooses whether right clicks in the named pane go to the pane or to the multiplexer.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone.
    pub(super) fn handle_pane_input_set(
        &mut self,
        params: &PaneInputSetParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(&params.pane_id)?;
        let state = self.state.topology.workspaces[workspace]
            .pane_state_mut(pane)
            .ok_or_else(|| pane_not_found(&params.pane_id))?;
        state.right_click_passthrough = params.right_click == PaneRightClickTarget::Pane;
        Ok(ResponseResult::Ok {})
    }

    /// Writes the text to the named pane's process exactly as given.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane without a process and `pane_send_failed` when the
    /// process cannot take the bytes.
    pub(super) fn handle_pane_send_text(
        &self,
        params: PaneSendTextParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let runtime = self.input_runtime(&params.pane_id)?;
        deliver(runtime, [params.text.into_bytes()])
    }

    /// Writes the text, framed as a paste when the process asked for that, and then the
    /// keys, as one write.
    ///
    /// # Errors
    /// Returns `invalid_key` before any byte is written when a key is not recognized, and
    /// otherwise the errors of sending text.
    pub(super) fn handle_pane_send_input(
        &self,
        params: PaneSendInputParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let runtime = self.input_runtime(&params.pane_id)?;
        let keys = encode_keys(runtime, &params.keys)?;
        let mut bytes = if params.text.is_empty() {
            Vec::new()
        } else {
            runtime.paste_payload(params.text)
        };
        bytes.extend(keys.into_iter().flatten());
        deliver(runtime, [bytes])
    }

    /// Writes each key to the named pane's process as its own write.
    ///
    /// # Errors
    /// Returns `invalid_key` before any byte is written when a key is not recognized, and
    /// otherwise the errors of sending text.
    pub(super) fn handle_pane_send_keys(
        &self,
        params: &PaneSendKeysParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let runtime = self.input_runtime(&params.pane_id)?;
        deliver(runtime, encode_keys(runtime, &params.keys)?)
    }

    /// The process of an explicitly named pane.
    fn input_runtime(&self, id: &str) -> Result<&PaneRuntime, ErrorBody> {
        let (workspace, pane) = self.target_pane(id)?;
        self.pane_runtime(workspace, pane)
            .ok_or_else(|| pane_not_found(id))
    }
}
