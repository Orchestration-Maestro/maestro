//! Wire protocol for maestro server/client communication.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
//!
//! Defines the message types, framing, version negotiation, and safety
//! constraints for the binary protocol over Unix domain sockets.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Protocol constants
// ---------------------------------------------------------------------------

/// Current protocol version. Bumped when wire format changes incompatibly.
pub use crate::PROTOCOL_VERSION;

/// Maximum allowed frame payload size (2 MB). Frames larger than this are
/// rejected to prevent denial-of-service via oversized length prefixes.
pub const MAX_FRAME_SIZE: usize = 2 * 1024 * 1024;

/// Maximum allowed server-to-client frame payload when Kitty graphics are enabled.
/// Normal traffic keeps `MAX_FRAME_SIZE`; this larger cap is only for explicit
/// image payloads that are naturally much larger after base64 encoding.
pub const MAX_GRAPHICS_FRAME_SIZE: usize = 32 * 1024 * 1024;

/// Maximum clipboard image payload size for remote paste bridging.
pub const MAX_CLIPBOARD_IMAGE_PAYLOAD: usize = 16 * 1024 * 1024;

/// Length of the u32 little-endian length prefix in bytes.
const LENGTH_PREFIX_BYTES: usize = 4;

/// Named modifier bits carried by structured input events.
pub struct KeyModifiers;

impl KeyModifiers {
    /// No modifier keys.
    pub const NONE: u8 = 0;
    /// Shift key.
    pub const SHIFT: u8 = 1;
    /// Control key.
    pub const CONTROL: u8 = 2;
    /// Alt key.
    pub const ALT: u8 = 4;
    /// Super key.
    pub const SUPER: u8 = 8;
    /// Hyper key.
    pub const HYPER: u8 = 16;
    /// Meta key.
    pub const META: u8 = 32;
}

// ---------------------------------------------------------------------------
// Client → Server messages
// ---------------------------------------------------------------------------

/// Render payload encoding negotiated during client handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderEncoding {
    /// Send full semantic `FrameData` values. This is the local/default mode.
    SemanticFrame,
    /// Send already-diffed terminal ANSI byte streams.
    TerminalAnsi,
}

/// Keybinding profile requested by an attached app client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientKeybindings {
    /// Use the server's own keybinding config.
    Server,
    /// Use this attached client's local action keymap; empty lists disable actions.
    Local {
        /// Action identifiers mapped to key sequences, including the prefix.
        bindings: std::collections::BTreeMap<String, Vec<String>>,
    },
}

/// Client behavior requested at connection time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientLaunchMode {
    /// Full app client.
    App,
    /// Full app client eligible for audited local direct graphics.
    AppDirectGraphics,
    /// Direct terminal attach client.
    TerminalAttach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Client key kind record.
pub enum ClientKeyKind {
    /// Press.
    Press,
    /// Repeat.
    Repeat,
    /// Release.
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Client key code record.
pub enum ClientKeyCode {
    /// Backspace.
    Backspace,
    /// Enter.
    Enter,
    /// Left.
    Left,
    /// Right.
    Right,
    /// Up.
    Up,
    /// Down.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Tab.
    Tab,
    /// Back tab.
    BackTab,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Esc.
    Esc,
    /// Char.
    Char(char),
    /// F.
    F(u8),
    /// Null.
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Client mouse button record.
pub enum ClientMouseButton {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Middle.
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Client mouse kind record.
pub enum ClientMouseKind {
    /// Down.
    Down(ClientMouseButton),
    /// Up.
    Up(ClientMouseButton),
    /// Drag.
    Drag(ClientMouseButton),
    /// Moved.
    Moved,
    /// Scroll up.
    ScrollUp,
    /// Scroll down.
    ScrollDown,
    /// Scroll left.
    ScrollLeft,
    /// Scroll right.
    ScrollRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Client input event record.
pub enum ClientInputEvent {
    /// Key.
    Key {
        /// Code.
        code: ClientKeyCode,
        /// Modifiers.
        modifiers: u8,
        /// Kind.
        kind: ClientKeyKind,
        /// Repeat count.
        repeat_count: u16,
        /// Generated text.
        generated_text: Option<String>,
        /// Source.
        source: ClientKeySource,
    },
    /// Text commit.
    TextCommit(String),
    /// Mouse.
    Mouse {
        /// Kind.
        kind: ClientMouseKind,
        /// Column.
        column: u16,
        /// Row.
        row: u16,
        /// Modifiers.
        modifiers: u8,
    },
    /// Paste.
    Paste {
        /// Text.
        text: String,
    },
    /// Focus gained.
    FocusGained,
    /// Focus lost.
    FocusLost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Client key source record.
pub enum ClientKeySource {
    /// Synthesized.
    Synthesized,
    /// Vt.
    Vt {
        /// Bytes.
        bytes: Vec<u8>,
    },
    /// Windows console.
    WindowsConsole {
        /// Record.
        record: crate::shared::WindowsKeyRecord,
    },
}

/// Messages sent from the client to the server over the client protocol socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Handshake: client announces its protocol version and terminal dimensions.
    Hello {
        /// Protocol version the client speaks.
        version: u32,
        /// Terminal width in columns.
        cols: u16,
        /// Terminal height in rows.
        rows: u16,
        /// Width of a terminal cell in physical pixels, or 0 when client-side Kitty graphics are disabled.
        cell_width_px: u32,
        /// Height of a terminal cell in physical pixels, or 0 when client-side Kitty graphics are disabled.
        cell_height_px: u32,
        /// Render encoding requested by the client.
        requested_encoding: RenderEncoding,
        /// Keybinding profile requested by the client.
        keybindings: ClientKeybindings,
        /// Whether this connection will render the full app or attach directly to a pane terminal.
        launch_mode: ClientLaunchMode,
    },

    /// Raw input bytes read from the client's stdin.
    Input {
        /// Raw terminal input (possibly multi-byte escape sequences).
        data: Vec<u8>,
    },

    /// Image bytes read from the client's local clipboard for remote paste bridging.
    ClipboardImage {
        /// Image file extension without a leading dot.
        extension: String,
        /// Raw image bytes.
        data: Vec<u8>,
    },

    /// Terminal resize notification from the client.
    Resize {
        /// New terminal width in columns.
        cols: u16,
        /// New terminal height in rows.
        rows: u16,
        /// Width of a terminal cell in physical pixels, or 0 when client-side Kitty graphics are disabled.
        cell_width_px: u32,
        /// Height of a terminal cell in physical pixels, or 0 when client-side Kitty graphics are disabled.
        cell_height_px: u32,
    },

    /// Graceful disconnect request.
    Detach,

    /// Switch this connection into direct terminal attach mode.
    AttachTerminal {
        /// Terminal id to attach to.
        terminal_id: String,
        /// Replace an existing writable attach owner for this terminal.
        takeover: bool,
    },

    /// Scroll input handled by a direct terminal attach client.
    AttachScroll {
        /// Original input source for routing.
        source: AttachScrollSource,
        /// Scroll direction.
        direction: AttachScrollDirection,
        /// Number of terminal rows to move when using host scrollback.
        lines: u16,
        /// Mouse column relative to the attached terminal, when available.
        column: Option<u16>,
        /// Mouse row relative to the attached terminal, when available.
        row: Option<u16>,
        /// Crossterm-compatible modifier bits for forwarded mouse wheel events.
        modifiers: u8,
    },

    /// Structured input events from platform clients that do not expose Unix-style raw bytes.
    InputEvents {
        /// Events.
        events: Vec<ClientInputEvent>,
    },

    /// Switch this connection into read-only terminal observe mode.
    ObserveTerminal {
        /// Pane, terminal, or agent target to observe.
        target: String,
    },

    /// Switch this connection into writable terminal control mode.
    ControlTerminal {
        /// Pane, terminal, or agent target to control.
        target: String,
        /// Replace an existing writable controller for this terminal.
        takeover: bool,
    },

    /// Result of the one armed Maestro-owned direct Kitty transmission.
    GraphicsTransmissionResult {
        /// Transfer id.
        transfer_id: u64,
        /// Image id.
        image_id: u32,
        /// Success.
        success: bool,
    },

    /// One confirmed SGR 1016 mouse report with read-time host geometry.
    InputPixels {
        /// Data.
        data: Vec<u8>,
        /// Cols.
        cols: u16,
        /// Rows.
        rows: u16,
        /// Width px.
        width_px: u32,
        /// Height px.
        height_px: u32,
    },

    /// The direct command was written and flushed; terminal response timing starts now.
    GraphicsTransmissionStarted {
        /// Transfer id.
        transfer_id: u64,
        /// Image id.
        image_id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Attach scroll direction record.
pub enum AttachScrollDirection {
    /// Up.
    Up,
    /// Down.
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Attach scroll source record.
pub enum AttachScrollSource {
    /// Wheel.
    Wheel,
    /// Page key.
    PageKey {
        /// Original key bytes to forward when the child application owns page keys.
        input: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Server → Client messages
// ---------------------------------------------------------------------------

/// A single cell in a rendered frame, serialized independently from ratatui's
/// `Cell` type to keep the wire protocol stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellData {
    /// Grapheme cluster displayed in this cell (usually 1–2 chars).
    pub symbol: String,
    /// Foreground color as a packed u32 (0xAARRGGBB or ratatui Color index).
    pub fg: u32,
    /// Background color as a packed u32.
    pub bg: u32,
    /// Bitmask of style modifiers (bold, italic, etc.) plus Maestro extension bits.
    pub modifier: u16,
    /// Whether this cell should be skipped during diff-based rendering.
    pub skip: bool,
    /// Index into `FrameData::hyperlinks` for this cell's OSC 8 target, if any.
    pub hyperlink: Option<u32>,
}

/// Cursor shape encoded as a DECSCUSR parameter.
///
/// 0 = terminal default, 1 = blinking block, 2 = steady block,
/// 3 = blinking underline, 4 = steady underline, 5 = blinking bar,
/// 6 = steady bar.
pub type CursorShapeParam = u8;

/// Cursor position within a rendered frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    /// Column offset (0-based) of the cursor.
    pub x: u16,
    /// Row offset (0-based) of the cursor.
    pub y: u16,
    /// Whether the cursor is visible.
    pub visible: bool,
    /// Cursor shape as a DECSCUSR parameter.
    #[serde(default)]
    pub shape: CursorShapeParam,
}

/// A rendered frame to be displayed by the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameData {
    /// Row-major cells supplied by the renderer.
    pub cells: Vec<CellData>,
    /// Frame width in columns.
    pub width: u16,
    /// Frame height in rows.
    pub height: u16,
    /// Cursor state for this frame, if applicable.
    pub cursor: Option<CursorState>,
    /// OSC 8 hyperlink URIs referenced by cells.
    pub hyperlinks: Vec<String>,
    /// Kitty graphics protocol bytes to apply after the text frame.
    pub graphics: Vec<u8>,
}

/// Terminal ANSI bytes encoded by the server for network-efficient clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFrame {
    /// Monotonic per-client frame sequence.
    pub seq: u64,
    /// Frame width in columns.
    pub width: u16,
    /// Frame height in rows.
    pub height: u16,
    /// Whether bytes contain a full redraw rather than an incremental diff.
    pub full: bool,
    /// Terminal escape bytes ready to write directly to stdout.
    pub bytes: Vec<u8>,
}

/// Notification kind forwarded from server to client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyKind {
    /// Play a sound (bell/agent-done, etc.).
    Sound,
    /// Display a toast message through the outer terminal.
    Toast,
    /// Display a toast message through the host OS notification service.
    SystemToast,
}

/// Messages sent from the server to the client over the client protocol socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Handshake response: server acknowledges (or rejects) the client.
    Welcome {
        /// Protocol version the server speaks.
        version: u32,
        /// Render encoding selected by the server for this connection.
        encoding: RenderEncoding,
        /// If present, the handshake failed and this describes why.
        /// The client should exit with a clear error message.
        error: Option<String>,
    },

    /// A rendered frame to be displayed by a semantic-frame client.
    Frame(FrameData),

    /// Terminal bytes to write directly for a terminal-ANSI client.
    Terminal(TerminalFrame),

    /// Client-local Kitty graphics bytes to write directly to the host terminal.
    Graphics {
        /// Raw Kitty graphics protocol bytes.
        bytes: Vec<u8>,
    },

    /// Server is shutting down. Clients should exit gracefully.
    ServerShutdown {
        /// Optional reason for the shutdown.
        reason: Option<String>,
    },

    /// A notification event (sound/toast) to be rendered locally by the client.
    Notify {
        /// What kind of notification.
        kind: NotifyKind,
        /// Human-readable title or sound label.
        message: String,
        /// Optional human-readable notification body.
        body: Option<String>,
    },

    /// OSC 52 clipboard data forwarded from a PTY through the server.
    Clipboard {
        /// Base64-encoded clipboard data.
        data: String,
    },

    /// Set the foreground client's outer terminal window title.
    WindowTitle {
        /// Sanitized title to write with OSC 0. `None` restores Maestro's default title.
        title: Option<String>,
    },

    /// Client-local runtime config changed on disk; refresh it without reconnecting.
    ReloadSoundConfig,

    /// Whether the client should currently capture host mouse input.
    MouseCapture {
        /// True when Maestro mouse UI is enabled or the focused pane app requests mouse reporting.
        enabled: bool,
        /// True only while the focused pane requests DEC SGR pixel mode 1016.
        sgr_pixels: bool,
    },

    /// Whether the focused terminal requests Kitty report-all keyboard input.
    KittyKeyboardReportAll {
        /// True only while the focused pane requests `REPORT_ALL_KEYS_AS_ESCAPE_CODES`.
        enabled: bool,
    },

    /// Apply the prefix-mode ASCII input-source change on the foreground client.
    /// `active = true` → switch to an ASCII-capable source (saving the current one);
    /// `active = false` → restore the saved source.
    PrefixInputSource {
        /// Whether the ASCII input source should be active.
        active: bool,
    },

    /// Ring the foreground client's outer terminal for pane-originated BEL characters.
    TerminalBell {
        /// Number of BEL characters parsed from one PTY read.
        count: u16,
    },

    /// One validated Maestro-owned Kitty regular-file RGBA transmission.
    GraphicsFile {
        /// Path.
        path: String,
        /// Expected len.
        expected_len: u64,
        /// Image id.
        image_id: u32,
        /// Transfer id.
        transfer_id: u64,
        /// Leading.
        leading: Vec<u8>,
        /// Control.
        control: String,
    },

    /// Suppress a direct command that expired before terminal delivery.
    GraphicsTransmissionRetired {
        /// Transfer id.
        transfer_id: u64,
        /// Image id.
        image_id: u32,
    },
}

// ---------------------------------------------------------------------------
// Framing: length-prefixed binary messages
// ---------------------------------------------------------------------------

/// Errors that can occur during framing operations.
#[derive(Debug)]
pub enum FramingError {
    /// The decoded payload length exceeds the configured maximum frame size.
    Oversized {
        /// Claimed.
        claimed: usize,
        /// Max.
        max: usize,
    },
    /// An I/O error occurred while reading or writing.
    Io(io::Error),
    /// Bincode serialization or deserialization failed.
    Bincode(String),
    /// The connection was closed before a complete frame could be read.
    UnexpectedEof,
}

impl std::fmt::Display for FramingError {
    /// Fmt.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FramingError::Oversized { claimed, max } => {
                write!(f, "frame size {claimed} exceeds maximum {max}")
            }
            FramingError::Io(e) => write!(f, "I/O error: {e}"),
            FramingError::Bincode(e) => write!(f, "bincode error: {e}"),
            FramingError::UnexpectedEof => write!(f, "unexpected end of stream"),
        }
    }
}

impl std::error::Error for FramingError {
    /// Source.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FramingError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for FramingError {
    /// From.
    fn from(e: io::Error) -> Self {
        FramingError::Io(e)
    }
}

/// Serializes a message and writes it as a length-prefixed frame:
/// `[u32LE length][bincode payload]`.
///
/// This is a blocking/synchronous write suitable for use with `std::os::unix::net::UnixStream`
/// in blocking mode, or with any `Write` implementor.
///
/// # Errors
///
/// Returns `FramingError::Bincode` if the payload length exceeds `u32::MAX`
/// (would be truncated by the length prefix cast).
pub fn write_message<W: Write, M: Serialize>(writer: &mut W, msg: &M) -> Result<(), FramingError> {
    let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())
        .map_err(|e| FramingError::Bincode(e.to_string()))?;

    let len = payload.len();
    let len = u32::try_from(len).map_err(|_| {
        FramingError::Bincode(format!(
            "payload length {len} exceeds u32::MAX ({}), would be truncated by length prefix",
            u32::MAX
        ))
    })?;

    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Reads and deserializes a length-prefixed frame from a reader.
///
/// Reassembles partial reads correctly. Rejects frames whose declared
/// length exceeds `max_frame_size` without panicking or allocating
/// oversized buffers.
///
/// # Errors
/// Returns an oversized, I/O, truncated-stream or binary decoding error.
pub fn read_message<R: Read, M: for<'de> Deserialize<'de>>(
    reader: &mut R,
    max_frame_size: usize,
) -> Result<M, FramingError> {
    // Read the 4-byte length prefix, reassembling partial reads.
    let mut len_buf = [0u8; LENGTH_PREFIX_BYTES];
    read_exact_or_eof(reader, &mut len_buf)?;
    let claimed_len = u32::from_le_bytes(len_buf) as usize;

    if claimed_len > max_frame_size {
        return Err(FramingError::Oversized {
            claimed: claimed_len,
            max: max_frame_size,
        });
    }

    // Read the payload, reassembling partial reads.
    let mut payload = vec![0u8; claimed_len];
    read_exact_or_eof(reader, &mut payload)?;

    let (msg, consumed) = bincode::serde::decode_from_slice(&payload, bincode::config::standard())
        .map_err(|e| FramingError::Bincode(e.to_string()))?;

    // Enforce that the decoder consumed the full payload.
    // Trailing bytes after the decoded message indicate a protocol violation
    // (e.g., a corrupted length prefix or concatenated payloads).
    if consumed != claimed_len {
        return Err(FramingError::Bincode(format!(
            "decoded {consumed} bytes but payload length was {claimed_len}; trailing bytes are not allowed"
        )));
    }

    Ok(msg)
}

/// Like `Read::read_exact`, but returns `FramingError::UnexpectedEof`
/// when the reader hits end-of-stream before filling the buffer, instead
/// of the generic `io::ErrorKind::UnexpectedEof`.
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<(), FramingError> {
    reader.read_exact(buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            FramingError::UnexpectedEof
        } else {
            FramingError::Io(e)
        }
    })
}

// ---------------------------------------------------------------------------
// Version negotiation
// ---------------------------------------------------------------------------

/// Result of checking a client's protocol version against the server's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionCheck {
    /// Versions are compatible. The server should reply with a successful Welcome.
    Compatible,
    /// Versions are incompatible. The server should reply with a Welcome error and close.
    Incompatible(String),
}

/// Checks whether a client's protocol version is compatible with this server.
///
/// Current rules:
/// - Version 0 (pre-persistence client) is always rejected.
/// - Exactly matching protocol versions are accepted.
/// - A client with a newer version than the server is rejected.
/// - A client with an older version than the server is rejected
///   (backward compatibility is not yet supported).
#[must_use]
pub fn check_client_version(client_version: u32) -> VersionCheck {
    if client_version == 0 {
        return VersionCheck::Incompatible(
            "pre-persistence client (version 0) is not supported".to_owned(),
        );
    }

    match client_version.cmp(&PROTOCOL_VERSION) {
        std::cmp::Ordering::Equal => VersionCheck::Compatible,
        std::cmp::Ordering::Less => VersionCheck::Incompatible(format!(
            "client version {client_version} is older than server version {PROTOCOL_VERSION}; please upgrade your maestro client"
        )),
        std::cmp::Ordering::Greater => VersionCheck::Incompatible(format!(
            "client version {client_version} is newer than server version {PROTOCOL_VERSION}; please upgrade the maestro server"
        )),
    }
}
