//! Byte-framed server-sent events, decoded only after each complete line.
use maestro_extensions_wasm::ExtensionResult;
use serde_json::Value;

/// One response's line and event framing state.
#[derive(Default)]
pub struct Decoder {
    /// Bytes before the next line separator, including split UTF-8 sequences.
    pending: Vec<u8>,
    /// Data fields awaiting a blank line.
    data: Vec<String>,
    /// Whether a following LF belongs to the preceding CR.
    after_cr: bool,
    /// Whether the first line has already been consumed.
    started: bool,
}

impl Decoder {
    /// Starts framing at the beginning of a response body.
    pub fn new() -> Self {
        Self::default()
    }

    /// Frames complete JSON data events without assuming native chunk boundaries.
    ///
    /// # Errors
    /// Returns malformed event JSON instead of accepting a partial payload.
    pub fn feed(&mut self, bytes: &[u8]) -> ExtensionResult<Vec<Value>> {
        let mut frames = Vec::new();
        for byte in bytes {
            let skip_lf = self.after_cr && *byte == b'\n';
            self.after_cr = false;
            if skip_lf {
                continue;
            }
            match *byte {
                b'\r' | b'\n' => {
                    self.after_cr = *byte == b'\r';
                    frames.extend(self.line()?);
                }
                byte => self.pending.push(byte),
            }
        }
        Ok(frames)
    }

    /// Handles one line, including comments, BOM and repeated data fields.
    fn line(&mut self) -> ExtensionResult<Option<Value>> {
        let bytes = std::mem::take(&mut self.pending);
        let text = String::from_utf8_lossy(&bytes);
        let line = if self.started {
            &text
        } else {
            self.started = true;
            text.strip_prefix('\u{feff}').unwrap_or(&text)
        };
        if line.is_empty() && !self.data.is_empty() {
            let data = std::mem::take(&mut self.data).join("\n");
            return serde_json::from_str(&data)
                .map(Some)
                .map_err(|error| error.to_string());
        }
        if let Some(data) = line.strip_prefix("data:") {
            self.data
                .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
        }
        Ok(None)
    }
}
