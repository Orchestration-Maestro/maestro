//! Local schema callbacks operating on a borrowed JSON value in its owning runtime.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{CallbackEmitter, ExtensionResult};
/// The shared error record returned by executable schema callbacks.
pub use maestro_request::diagnostics::DiagnosticErrorInfo;
/// A schema operation with its original structured failure.
pub type SchemaResult<T> = Result<T, DiagnosticErrorInfo>;
use serde_json::Value;
use std::{fmt, rc::Rc};

/// The kind of one JSON value.
pub use crate::bindings::maestro::extension::host::JsonKind;

/// The operations on an invocation-local JSON operand.
pub(crate) trait JsonPort {
    /// Borrow this runtime's own invocation resource without copying its JSON.
    #[cfg(any(test, target_arch = "wasm32"))]
    fn resource(&self) -> &dyn std::any::Any;

    /// Inspect a value without copying its contents.
    fn kind(&self, path: &str) -> ExtensionResult<Option<JsonKind>>;
    /// Read one explicitly requested value; absence is distinct from null.
    fn read(&self, path: &str) -> ExtensionResult<Option<Value>>;
    /// Enumerate object keys without copying their values.
    fn keys(&self, path: &str) -> ExtensionResult<Vec<String>>;
    /// Assign a member of the selected container.
    fn set(&self, path: &str, key: &str, value: &Value) -> ExtensionResult<()>;
    /// Delete a member of the selected container.
    fn delete(&self, path: &str, key: &str) -> ExtensionResult<()>;
    /// Edit the selected array, returning the removed members.
    fn splice(
        &self,
        path: &str,
        start: u32,
        delete: u32,
        values: &[Value],
    ) -> ExtensionResult<Vec<Value>>;
}

/// A borrowed view of the current JSON operand.
///
/// Reads copy only values the author explicitly requests. Member views borrow this view;
/// creating or dropping one neither copies nor removes its value. Assigning a local view
/// variable does not replace an invocation argument.
pub struct JsonView<'a> {
    /// The active invocation's native value capability.
    pub(crate) port: &'a dyn JsonPort,
    /// Author-selected path within this operand, never within the checker root.
    pub(crate) path: String,
}
#[cfg(any(test, target_arch = "wasm32"))]
impl<'a> JsonView<'a> {
    /// Borrow the supplied invocation operand without reading or encoding it.
    pub(crate) fn new(port: &'a dyn JsonPort) -> Self {
        Self {
            port,
            path: String::new(),
        }
    }
}
impl JsonView<'_> {
    /// Borrow a member or array index by its JSON key, including an absent member.
    #[must_use]
    pub fn member(&mut self, key: &str) -> JsonView<'_> {
        JsonView {
            port: self.port,
            path: format!("{}/{}", self.path, maestro_json::escape_pointer_token(key)),
        }
    }
    /// Inspect the selected kind without copying the value.
    ///
    /// # Errors
    /// Returns the host's access error.
    pub fn kind(&self) -> SchemaResult<Option<JsonKind>> {
        self.port
            .kind(&self.path)
            .map_err(DiagnosticErrorInfo::error)
    }
    /// Read the explicitly selected value. Missing members remain absent, not null.
    ///
    /// # Errors
    /// Returns the host's read error.
    pub fn read(&self) -> SchemaResult<Option<Value>> {
        self.port
            .read(&self.path)
            .map_err(DiagnosticErrorInfo::error)
    }
    /// Enumerate this object's keys in their observed order.
    ///
    /// # Errors
    /// Returns the host's error when the operand is absent or not an object.
    pub fn keys(&self) -> SchemaResult<Vec<String>> {
        self.port
            .keys(&self.path)
            .map_err(DiagnosticErrorInfo::error)
    }
    /// Assign an object member or array index without replacing this view's own slot.
    ///
    /// # Errors
    /// Returns the host's error when the container or key does not admit assignment.
    pub fn set(&mut self, key: &str, value: &Value) -> SchemaResult<()> {
        self.port
            .set(&self.path, key, value)
            .map_err(DiagnosticErrorInfo::error)
    }
    /// Delete an object member, or clear an array position to JSON null.
    ///
    /// # Errors
    /// Returns the host's error when the container or key does not admit deletion.
    pub fn delete(&mut self, key: &str) -> SchemaResult<()> {
        self.port
            .delete(&self.path, key)
            .map_err(DiagnosticErrorInfo::error)
    }
    /// Replace a range of this array, returning the removed members in order.
    ///
    /// # Errors
    /// Returns the host's error when the selected value is not an array.
    pub fn splice(
        &mut self,
        start: u32,
        delete: u32,
        values: &[Value],
    ) -> SchemaResult<Vec<Value>> {
        self.port
            .splice(&self.path, start, delete, values)
            .map_err(DiagnosticErrorInfo::error)
    }
}

/// An author callback over a borrowed JSON view and its originating emitter.
type Callback<R> = dyn for<'a, 'b> Fn(&mut JsonView<'a>, CallbackEmitter<'b>) -> SchemaResult<R>;
/// A local schema callback retaining its author captures.
pub struct SchemaCallback<R>(maestro_request::parameter_schema::SchemaCallback<R, Callback<R>>);
impl<R> Clone for SchemaCallback<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<R> fmt::Debug for SchemaCallback<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<R> PartialEq for SchemaCallback<R> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl<R> SchemaCallback<R> {
    /// Retain an author callback without invoking it.
    pub fn new(
        callback: impl for<'a, 'b> Fn(&mut JsonView<'a>, CallbackEmitter<'b>) -> SchemaResult<R>
        + 'static,
    ) -> Self {
        Self(maestro_request::parameter_schema::SchemaCallback::from_callback(Rc::new(callback)))
    }
    /// Invoke the author callback against the current borrowed operand.
    ///
    /// # Errors
    /// Returns the author's original failure message.
    pub fn call(&self, value: &mut JsonView<'_>, emitter: CallbackEmitter<'_>) -> SchemaResult<R> {
        (self.0)(value, emitter)
    }
}
impl Default for SchemaCallback<Option<Value>> {
    fn default() -> Self {
        Self::new(|_, _| Ok(None))
    }
}
impl Default for SchemaCallback<bool> {
    fn default() -> Self {
        Self::new(|_, _| Ok(true))
    }
}
impl Default for SchemaCallback<Vec<Value>> {
    fn default() -> Self {
        Self::new(|_, _| Ok(Vec::new()))
    }
}
/// A schema's conversion, guard and guard-error callbacks.
pub type Base = maestro_request::parameter_schema::Base<
    SchemaCallback<Option<Value>>,
    SchemaCallback<bool>,
    SchemaCallback<Vec<Value>>,
>;
/// One ordered refinement and its message producer.
pub type Refinement =
    maestro_request::parameter_schema::Refinement<SchemaCallback<bool>, SchemaCallback<String>>;
/// Executable callbacks at one schema location.
pub type SchemaNode = maestro_request::parameter_schema::SchemaNode<Base, Refinement>;
/// Executable nodes keyed by the author's schema pointers.
pub type SchemaCode = maestro_request::parameter_schema::SchemaCode<Base, Refinement>;
/// One existing callback-table entry retaining all callbacks at one location.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) type Handler = Rc<SchemaNode>;
