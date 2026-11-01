//! Callback-bearing node records for local executable schemas.
use crate::diagnostics::DiagnosticErrorInfo;
use serde_json::Value;
use std::{collections::HashMap, fmt, marker::PhantomData, ops::Deref, rc::Rc};
/// The fallible operation supplied by a schema author.
type Callback<R> = dyn Fn(&mut Value) -> Result<R, DiagnosticErrorInfo>;

/// An author callback retaining its captures locally.
///
/// The default callable receives mutable JSON and returns `R`. Adapters select
/// their own callable signature while reusing capture ownership and identity.
pub struct SchemaCallback<R, F: ?Sized = Callback<R>> {
    /// The one shared capture owner for either callback signature.
    callback: Rc<F>,
    /// Tracks the result type without retaining a result value.
    result: PhantomData<fn() -> R>,
}
impl<R, F: ?Sized> Clone for SchemaCallback<R, F> {
    fn clone(&self) -> Self {
        Self {
            callback: Rc::clone(&self.callback),
            result: PhantomData,
        }
    }
}
impl<R, F: ?Sized> fmt::Debug for SchemaCallback<R, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SchemaCallback")
    }
}
impl<R, F: ?Sized> PartialEq for SchemaCallback<R, F> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.callback, &other.callback)
    }
}
impl<R, F: ?Sized> SchemaCallback<R, F> {
    /// Retain an already shared implementation without invoking it.
    ///
    /// The callable type determines the invocation signature.
    pub fn from_callback(callback: Rc<F>) -> Self {
        Self {
            callback,
            result: PhantomData,
        }
    }
}
impl<R, F: ?Sized> Deref for SchemaCallback<R, F> {
    type Target = F;
    fn deref(&self) -> &F {
        self.callback.as_ref()
    }
}
impl<R> SchemaCallback<R> {
    /// Retain one author callback.
    pub fn new(callback: impl Fn(&mut Value) -> Result<R, DiagnosticErrorInfo> + 'static) -> Self {
        Self::from_callback(Rc::new(callback))
    }
    /// Invoke the callback outside all tool and registration guards.
    ///
    /// # Errors
    /// Returns the author's original error when the callback fails.
    pub fn call(&self, value: &mut Value) -> Result<R, DiagnosticErrorInfo> {
        (self.callback)(value)
    }
}
impl Default for SchemaCallback<Option<Value>> {
    fn default() -> Self {
        Self::new(|_| Ok(None))
    }
}
impl Default for SchemaCallback<bool> {
    fn default() -> Self {
        Self::new(|_| Ok(true))
    }
}
impl Default for SchemaCallback<Vec<Value>> {
    fn default() -> Self {
        Self::new(|_| Ok(Vec::new()))
    }
}
/// A schema with its own conversion, guard and error producer.
#[derive(Clone, Debug, PartialEq)]
pub struct Base<
    C = SchemaCallback<Option<Value>>,
    K = SchemaCallback<bool>,
    E = SchemaCallback<Vec<Value>>,
> {
    /// A replacement result or only in-place edits.
    pub convert: C,
    /// Acceptance of the current value.
    pub check: K,
    /// Guard details requested only when the guard fails.
    pub errors: E,
}
impl<C: Default, K: Default, E: Default> Default for Base<C, K, E> {
    fn default() -> Self {
        Self {
            convert: C::default(),
            check: K::default(),
            errors: E::default(),
        }
    }
}
/// A custom check applied after the ordinary schema succeeds.
#[derive(Clone, Debug, PartialEq)]
pub struct Refinement<K = SchemaCallback<bool>, E = SchemaCallback<String>> {
    /// Acceptance of the current value.
    pub check: K,
    /// Error message for a failed check.
    pub error: E,
}
/// Executable annotations at one declared schema location.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaNode<B = Base, R = Refinement> {
    /// Optional base conversion and guard.
    pub base: Option<B>,
    /// Refinements in registration order.
    pub refinements: Vec<R>,
}
/// Executable nodes keyed by their supplied schema's JSON pointers.
pub type SchemaCode<B = Base, R = Refinement> = HashMap<String, SchemaNode<B, R>>;
/// The two schema payloads the validator consumes.
pub trait SchemaCodeSource {
    /// Borrow executable annotations, or none for an ordinary schema.
    fn schema_code(&self) -> Option<&SchemaCode>;
}
impl SchemaCodeSource for () {
    fn schema_code(&self) -> Option<&SchemaCode> {
        None
    }
}
impl SchemaCodeSource for SchemaCode {
    fn schema_code(&self) -> Option<&SchemaCode> {
        Some(self)
    }
}
