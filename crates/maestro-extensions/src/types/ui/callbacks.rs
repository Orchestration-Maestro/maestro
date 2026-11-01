//! Typed UI callbacks retaining their originating event capability.
use crate::{CallbackEmitter, EventError, ExtensionAPI, ExtensionExecution};
use std::{
    hash::{Hash, Hasher},
    rc::{Rc, Weak},
};

/// An extension UI callback bound to the facade that authored it.
///
/// A host invokes [`UIHostCall::call`] with the callback's operands. The
/// retained originating bus enters the existing callback frame around the
/// callback, so same-extension listeners run inside the call and foreign
/// listeners run after it returns. Cloning shares the callback allocation
/// without requiring `A: Clone` or `R: Clone`.
pub struct UIHostCall<A, R> {
    /// The callback and its event capability, erased together.
    callback: Rc<dyn Fn(A) -> Result<R, EventError>>,
}

impl<A, R> Clone for UIHostCall<A, R> {
    fn clone(&self) -> Self {
        Self {
            callback: Rc::clone(&self.callback),
        }
    }
}

impl<A, R> UIHostCall<A, R> {
    /// Retains host glue that invokes an already bound extension callback.
    pub(super) fn from_host(callback: impl Fn(A) -> Result<R, EventError> + 'static) -> Self {
        Self {
            callback: Rc::new(callback),
        }
    }

    /// Invokes the original callback with its retained event capability.
    ///
    /// # Errors
    /// Returns the original callback failure.
    pub fn call(&self, args: A) -> Result<R, EventError> {
        (self.callback)(args)
    }

    /// Whether two handles retain the same callback allocation.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.callback, &other.callback)
    }

    /// A reference to the same callback that does not keep it alive.
    #[must_use]
    pub fn downgrade(&self) -> WeakUIHostCall<A, R> {
        WeakUIHostCall {
            callback: Rc::downgrade(&self.callback),
        }
    }
}

/// A reference to a UI callback that does not keep the callback alive.
///
/// While it exists, no other callback is allocated at its address, so the address identifies
/// the callback until it is dropped.
pub struct WeakUIHostCall<A, R> {
    /// The callback allocation.
    callback: Weak<dyn Fn(A) -> Result<R, EventError>>,
}

impl<A, R> WeakUIHostCall<A, R> {
    /// The address of the callback allocation.
    #[must_use]
    pub fn addr(&self) -> usize {
        self.callback.as_ptr().cast::<()>().addr()
    }

    /// Whether a handle still retains the callback.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.callback.strong_count() > 0
    }
}

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Binds a synchronous UI callback to this facade's event capability.
    ///
    /// The borrowed emitter lives only for the call; keeping it afterwards
    /// does not compile, while [`CallbackEmitter::events`] still supplies an
    /// owned emitter a tail may retain:
    ///
    /// ```compile_fail,E0521
    /// use maestro_extensions::{CallbackEmitter, ExtensionAPI};
    /// use maestro_models::Local;
    /// use std::{cell::RefCell, rc::Rc};
    ///
    /// fn callback_emitter_cannot_escape_a_bound_ui_call(api: &ExtensionAPI<Local>) {
    ///     let escaped: Rc<RefCell<Option<CallbackEmitter<'static, Local>>>> = Rc::default();
    ///     let _ = api.ui_callback(move |(), emitter| {
    ///         escaped.borrow_mut().replace(emitter);
    ///         Ok(())
    ///     });
    /// }
    /// ```
    pub fn ui_callback<A: 'static, R: 'static>(
        &self,
        callback: impl for<'a> Fn(A, CallbackEmitter<'a, E>) -> Result<R, EventError> + 'static,
    ) -> UIHostCall<A, R> {
        let events = self.events.clone();
        UIHostCall::from_host(move |args| events.invoke_callback(|emitter| callback(args, emitter)))
    }
}

impl<A, R> PartialEq for UIHostCall<A, R> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}
impl<A, R> Eq for UIHostCall<A, R> {}
impl<A, R> Hash for UIHostCall<A, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.callback).cast::<()>().hash(state);
    }
}
