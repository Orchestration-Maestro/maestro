//! Closed ownership and future representations shared by callable owners.
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Weak as ThreadedWeak};
use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
    rc::{Rc, Weak as LocalWeak},
};
/// Caller-local owning pointers and futures.
#[derive(Clone, Copy, Default)]
pub struct Local;
/// Existing SDK representation, thread-safe on native targets.
#[derive(Clone, Copy, Default)]
pub struct Threaded;
/// Prevent external execution representations.
mod sealed {
    /// The two supported representations.
    pub trait Sealed {}
    impl Sealed for super::Local {}
    impl Sealed for super::Threaded {}
}
/// Closed pointer and future representation for caller-owned execution.
pub trait Execution: sealed::Sealed + Sized + 'static {
    /// An erased operation future.
    type Future<'a, T>: Future<Output = T> + Unpin;
    /// The owning pointer.
    type Shared<T: ?Sized>: Clone + Deref<Target = T>;
    /// The non-owning pointer.
    type Weak<T: ?Sized>;
    /// Construct the owning pointer.
    fn share<T>(value: T) -> Self::Shared<T>;
}
/// One representation template with only pointer and auto-trait differences.
macro_rules! family {
    ($mode:ty,$pointer:ident,$weak:ident,[$($bound:tt)*])=>{
impl Execution for $mode {
    type Future<'a, T> = Pin<Box<dyn Future<Output = T> $($bound)* + 'a>>;
    type Shared<T: ?Sized> = $pointer<T>;
    type Weak<T: ?Sized> = $weak<T>;
    fn share<T>(value: T) -> Self::Shared<T> {
        $pointer::new(value)
    }
}
    };
}
family!(Local, Rc, LocalWeak, []);
#[cfg(not(target_arch = "wasm32"))]
family!(Threaded,Arc,ThreadedWeak,[+Send]);
#[cfg(target_arch = "wasm32")]
family!(Threaded, Rc, LocalWeak, []);
