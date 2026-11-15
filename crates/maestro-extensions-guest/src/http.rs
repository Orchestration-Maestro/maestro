//! Owned HTTP requests and independently retained native responses.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::AbortSignal;

/// One HTTP attempt with an optional caller-owned cancellation signal.
pub struct Request {
    /// Supplied URL.
    pub url: String,
    /// Omitted selects GET.
    pub method: Option<String>,
    /// Supplied header pairs, normalized by the native HTTP owner.
    pub headers: Vec<(String, String)>,
    /// Absent and present-empty request bodies remain distinct.
    pub body: Option<Vec<u8>>,
    /// Original caller signal, retained while response work remains live.
    pub signal: Option<AbortSignal>,
}
/// Independently retained response from one native HTTP attempt.
pub trait Response {
    /// Consumes the original unused body as any JSON value through the native parser.
    fn json(&self) -> crate::ExtensionFuture<'_, serde_json::Value>;
    /// Native HTTP status code, including non-success responses.
    fn status(&self) -> u16;
    /// Consumes the original unused body as BOM-aware, lossy UTF-8 text.
    fn text(&self) -> crate::ExtensionFuture<'_, String>;
    /// Whether the status is within 200 through 299 inclusive.
    fn ok(&self) -> bool {
        (200..300).contains(&self.status())
    }
    /// Original native reason phrase.
    fn status_text(&self) -> String;
    /// Native HTTP owner's normalized response headers.
    fn headers(&self) -> Vec<(String, String)>;
    /// An alias of the original body; obtaining it does not consume bytes.
    fn body(&self) -> Option<std::rc::Rc<dyn crate::streams::Readable>>;
}
