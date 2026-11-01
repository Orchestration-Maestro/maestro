//! Canonical component records for executable schema calls.
use wasmtime::component::{ComponentType, Lift, Lower, Resource};
/// The canonical kind of a present JSON value.
#[derive(ComponentType, Lift, Lower, Clone, Copy, Debug)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum JsonKind {
    /// JSON null.
    #[component(name = "null")]
    Null,
    /// A JSON boolean.
    #[component(name = "boolean")]
    Boolean,
    /// A JSON number.
    #[component(name = "number")]
    Number,
    /// A JSON string.
    #[component(name = "string")]
    String,
    /// A JSON array.
    #[component(name = "array")]
    Array,
    /// A JSON object.
    #[component(name = "object")]
    Object,
}
/// The selected schema callback method.
#[derive(ComponentType, Lift, Lower, Clone, Debug)]
#[component(variant)]
pub(crate) enum Operation {
    /// Conversion callback.
    #[component(name = "convert")]
    Convert,
    /// The shared native acceptance callback signature.
    #[component(name = "check")]
    Check,
    /// Base diagnostic callback.
    #[component(name = "errors")]
    Errors,
    /// Indexed refinement check.
    #[component(name = "refine")]
    Refine(u32),
    /// Indexed refinement message.
    #[component(name = "message")]
    Message(u32),
}
/// The methods provided by one guest schema node.
#[derive(ComponentType, Lift, Lower, Clone, Copy, Debug)]
#[component(record)]
pub(crate) struct Shape {
    /// Whether this node supplies Base methods.
    pub(crate) base: bool,
    /// The number of ordered refinement slots.
    pub(crate) refinements: u32,
}
/// The original textual or numeric diagnostic code.
#[derive(ComponentType, Lift, Lower, Debug)]
#[component(variant)]
pub(crate) enum Code {
    /// Textual diagnostic code.
    #[component(name = "text")]
    Text(String),
    /// A JSON number.
    #[component(name = "number")]
    Number(f64),
}
/// The guest failure without loss of its supplied fields.
#[derive(ComponentType, Lift, Lower, Debug)]
#[component(record)]
pub(crate) struct Error {
    /// The optional original error name.
    pub(crate) name: Option<String>,
    /// The original error message.
    pub(crate) message: String,
    /// The optional authored stack.
    pub(crate) stack: Option<String>,
    /// The optional original error code.
    pub(crate) code: Option<Code>,
}
impl From<Error> for maestro_extensions::DiagnosticErrorInfo {
    /// The original canonical from operation.
    fn from(e: Error) -> Self {
        Self {
            name: e.name,
            message: e.message,
            stack: e.stack,
            code: e.code.map(|c| match c {
                Code::Text(s) => maestro_extensions::DiagnosticCode::Text(s),
                Code::Number(n) => maestro_extensions::DiagnosticCode::Number(n),
            }),
        }
    }
}
/// A retained schema identity for one enumerated declaration.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Binding {
    /// The metadata declaration index.
    pub(crate) tool: u32,
    /// The authored schema location.
    pub(crate) pointer: String,
    /// The original retained callback identity.
    pub(crate) handler: Resource<crate::guest_calls::Callback>,
    /// The original callable shape.
    pub(crate) shape: Shape,
    /// Whether the author lives in the receiving instance.
    pub(crate) local: bool,
}
/// Live declaration metadata with independently retained schema code.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Listing {
    /// The complete schema metadata document.
    pub(crate) metadata: String,
    /// The schema bindings admitted for this declaration.
    pub(crate) schemas: Vec<Binding>,
}

impl From<maestro_extensions::DiagnosticErrorInfo> for Error {
    /// Preserve every authored diagnostic field across schema mediation.
    fn from(error: maestro_extensions::DiagnosticErrorInfo) -> Self {
        Self {
            name: error.name,
            message: error.message,
            stack: error.stack,
            code: error.code.map(|code| match code {
                maestro_extensions::DiagnosticCode::Text(value) => Code::Text(value),
                maestro_extensions::DiagnosticCode::Number(value) => Code::Number(value),
            }),
        }
    }
}
