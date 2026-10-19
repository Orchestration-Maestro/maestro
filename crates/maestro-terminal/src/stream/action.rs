//! Typed operations emitted by the terminal byte stream.

use crate::{CharsetActiveSlot, CharsetSlot};

/// One decoded terminal operation.
///
/// Parameter views borrow the parser only for the synchronous handler call.
/// Operating-system command payloads retain their existing owned representation.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Print one Unicode scalar.
    Print(char),
    /// Advance one row, scrolling at the lower margin.
    Index,
    /// Request the terminal's answerback.
    Enquiry,
    /// Signal an audible or visual bell.
    Bell,
    /// Move one column backward.
    Backspace,
    /// Advance through the specified number of tab destinations.
    HorizontalTab(u16),
    /// Advance to the following line.
    Linefeed,
    /// Move to the first column.
    CarriageReturn,
    /// Select a row and column, retaining protocol zero values.
    CursorPos {
        /// Requested row.
        row: u16,
        /// Requested column.
        col: u16,
    },
    /// Move up by the specified count.
    CursorUp(u16),
    /// Move down by the specified count.
    CursorDown(u16),
    /// Move left by the specified count.
    CursorLeft(u16),
    /// Move right by the specified count.
    CursorRight(u16),
    /// Select an absolute column.
    CursorCol(u16),
    /// Select an absolute row.
    CursorRow(u16),
    /// Advance the column by the specified count.
    CursorColRelative(u16),
    /// Advance the row by the specified count.
    CursorRowRelative(u16),
    /// Move backward through tab destinations.
    HorizontalTabBack(u16),
    /// Insert blank lines.
    InsertLines(u16),
    /// Delete lines.
    DeleteLines(u16),
    /// Insert blank character cells.
    InsertBlanks(u16),
    /// Delete character cells.
    DeleteChars(u16),
    /// Erase character cells.
    EraseChars(u16),
    /// Scroll rows upward.
    ScrollUp(u16),
    /// Scroll rows downward.
    ScrollDown(u16),
    /// Repeat the previous printed character.
    PrintRepeat(u16),
    /// Set a tab destination at the current column.
    TabSet,
    /// Clear the current column's tab destination.
    TabClearCurrent,
    /// Clear every tab destination.
    TabClearAll,
    /// Restore the default tab destinations.
    TabReset,
    /// Erase the display below the cursor, respecting protection when requested.
    EraseDisplayBelow(bool),
    /// Erase the display above the cursor, respecting protection when requested.
    EraseDisplayAbove(bool),
    /// Erase the complete display, respecting protection when requested.
    EraseDisplayComplete(bool),
    /// Erase scrollback, respecting protection when requested.
    EraseDisplayScrollback(bool),
    /// Scroll the complete display into history, respecting protection when requested.
    EraseDisplayScrollComplete(bool),
    /// Erase rightward in the row, respecting protection when requested.
    EraseLineRight(bool),
    /// Erase leftward in the row, respecting protection when requested.
    EraseLineLeft(bool),
    /// Erase the complete row, respecting protection when requested.
    EraseLineComplete(bool),
    /// Configure a character-set slot.
    ConfigureCharset {
        /// Destination slot.
        slot: CharsetSlot,
        /// Character mapping to install.
        charset: crate::Charset,
    },
    /// Save the active cursor.
    SaveCursor,
    /// Restore the saved cursor.
    RestoreCursor,
    /// Fill the active screen with alignment characters.
    Decaln,
    /// Advance a row and return to the first column.
    NextLine,
    /// Move upward, scrolling at the top margin.
    ReverseIndex,
    /// Enable ISO character protection.
    ProtectedModeIso,
    /// Disable character protection.
    ProtectedModeOff,
    /// Request a device-attribute report.
    DeviceAttributes(crate::device_attributes::Req),
    /// Restore terminal defaults.
    FullReset,
    /// Enable a terminal mode.
    SetMode(crate::Mode),
    /// Disable a terminal mode.
    ResetMode(crate::Mode),
    /// Save a terminal mode's current value.
    SaveMode(crate::Mode),
    /// Restore a terminal mode's saved value.
    RestoreMode(crate::Mode),
    /// Request a supported terminal mode's status.
    RequestMode(crate::Mode),
    /// Request the status of an unrecognized numeric mode.
    RequestModeUnknown(crate::modes::ModeTag),
    /// Select keyboard escape-sequence encoding.
    ModifyKeyFormat(crate::ansi::ModifyKeyFormat),
    /// Query current keyboard protocol flags.
    KittyKeyboardQuery,
    /// Push keyboard protocol flags.
    KittyKeyboardPush(crate::kitty::key::Flags),
    /// Pop keyboard protocol history entries.
    KittyKeyboardPop(u16),
    /// Replace keyboard protocol flags.
    KittyKeyboardSet(crate::kitty::key::Flags),
    /// Enable keyboard protocol flags.
    KittyKeyboardSetOr(crate::kitty::key::Flags),
    /// Disable keyboard protocol flags.
    KittyKeyboardSetNot(crate::kitty::key::Flags),
    /// Request device status.
    DeviceStatus(crate::device_status::Request),
    /// Request terminal version information.
    Xtversion,
    /// Request window information.
    SizeReport(crate::csi::SizeReportStyle),
    /// Select the active text display.
    ActiveStatusDisplay(crate::ansi::StatusDisplay),
    /// Choose whether mouse handling captures the shift modifier.
    MouseShiftCapture(bool),
    /// Select cursor appearance.
    CursorStyle(crate::ansi::CursorStyle),
    /// Enable DEC character protection.
    ProtectedModeDec,
    /// Apply an attribute borrowed from the current parameter list.
    SetAttribute(crate::sgr::Attribute<'a>),
    /// Set vertical scrolling margins.
    TopAndBottomMargin {
        /// First margin operand.
        top_left: u16,
        /// Second margin operand.
        bottom_right: u16,
    },
    /// Set horizontal scrolling margins.
    LeftAndRightMargin {
        /// First margin operand.
        top_left: u16,
        /// Second margin operand.
        bottom_right: u16,
    },
    /// Save the cursor or reset horizontal margins according to terminal mode.
    LeftAndRightMarginAmbiguous,
    /// Push a window-title index.
    TitlePush(u16),
    /// Pop a window-title index.
    TitlePop(u16),
    /// Begin a device-control payload with a callback-borrowed header.
    DcsHook(crate::parser::Dcs<'a>),
    /// Deliver one device-control payload byte.
    DcsPut(u8),
    /// End the device-control payload.
    DcsUnhook,
    /// Begin an application-command payload.
    ApcStart,
    /// Deliver one application-command payload byte.
    ApcPut(u8),
    /// End the application-command payload.
    ApcEnd,
    /// Apply an owned semantic prompt command.
    SemanticPrompt(crate::osc::SemanticPrompt),
    /// Set a UTF-8-validated window title.
    WindowTitle(Vec<u8>),
    /// Deliver an undecoded selection payload.
    ClipboardContents {
        /// Selection byte.
        kind: u8,
        /// Undecoded contents or query.
        data: Vec<u8>,
    },
    /// Report unvalidated directory bytes.
    ReportPwd(Vec<u8>),
    /// Start a hyperlink with owned option and URI bytes.
    StartHyperlink {
        /// Optional identifier.
        id: Option<Vec<u8>>,
        /// Unvalidated URI.
        uri: Vec<u8>,
    },
    /// End the active hyperlink.
    EndHyperlink,
    /// Deliver owned color requests in their original order.
    ColorOperation {
        /// Selected color operation.
        op: crate::osc::color::Operation,
        /// Ordered color requests.
        requests: crate::osc::color::List,
        /// Requested reply terminator.
        terminator: crate::osc::Terminator,
    },
    /// Deliver owned color-protocol requests.
    KittyColorReport(crate::kitty::color::Osc),
    /// Select a character-set bank.
    InvokeCharset {
        /// Left or right bank.
        bank: CharsetActiveSlot,
        /// Character-set slot to invoke.
        charset: CharsetSlot,
        /// Whether to use the one-character shift.
        locking: bool,
    },
}
