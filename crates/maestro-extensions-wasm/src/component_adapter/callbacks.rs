//! The private table that keeps author closures, keyed by the identity of the host-owned
//! callback resource that announces each of them.
//!
//! Author code and destructors never run while the table is borrowed: entries leave the
//! table first and drop afterwards, so a destructor may register another entry.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use super::imports::Imports;
use crate::types::{CommandHandler, ExtensionHandler, ExtensionResult, WithSession};

/// Declares the closure kinds the table keeps. Shared closures are cloned out of their entry;
/// one-shot closures run once, so they are taken instead.
macro_rules! kinds {
    (
        shared { $($shared:ident($shared_closure:ty),)* }
        once { $($once:ident($once_closure:ty),)* }
    ) => {
        /// A closure the table keeps for one host identity.
        pub(super) enum Kind {
            $(
                /// A closure the host announced under one identity.
                $shared($shared_closure),
            )*
            $(
                /// A closure that runs at most once; taken when it runs.
                $once(Option<$once_closure>),
            )*
        }

        /// A closure type the table stores under its own kind.
        pub(super) trait Stored: Sized {
            /// Wraps the closure in its table kind.
            fn wrap(self) -> Kind;
        }

        /// A closure type that is cloned out of its entry.
        pub(super) trait Shared: Stored {
            /// Clones the closure out of a table entry of the matching kind.
            fn pick(kind: &Kind) -> Option<Self>;
        }

        /// A closure type that is taken out of its entry.
        pub(super) trait Once: Stored {
            /// Takes the closure out of a table entry of the matching kind.
            fn take(kind: &mut Kind) -> Option<Self>;
        }

        $(
            impl Stored for $shared_closure {
                fn wrap(self) -> Kind {
                    Kind::$shared(self)
                }
            }

            impl Shared for $shared_closure {
                fn pick(kind: &Kind) -> Option<Self> {
                    match kind {
                        Kind::$shared(closure) => Some(closure.clone()),
                        _ => None,
                    }
                }
            }
        )*

        $(
            impl Stored for $once_closure {
                fn wrap(self) -> Kind {
                    Kind::$once(Some(self))
                }
            }

            impl Once for $once_closure {
                fn take(kind: &mut Kind) -> Option<Self> {
                    match kind {
                        Kind::$once(slot) => slot.take(),
                        _ => None,
                    }
                }
            }
        )*
    };
}

kinds! {
    shared {
        Event(ExtensionHandler),
        Command(CommandHandler),
    }
    once {
        WithSession(WithSession),
    }
}

/// One table entry: the closure and, for registered callbacks, the owner handle that keeps
/// the host identity alive until the entry is released. The closure drops before the handle.
struct Entry {
    /// The closure.
    kind: Kind,
    /// The owner handle of a registered callback; operation-scoped entries hold theirs in
    /// [`Scoped`].
    _owner: Option<Box<dyn Any>>,
}

thread_local! {
    /// Closures by host identity.
    static TABLE: RefCell<HashMap<u32, Entry>> = RefCell::new(HashMap::new());
}

/// A host identity that is not in the table yet.
pub(super) struct Identity<I: Imports> {
    /// The owner handle created by the host.
    pub(super) handle: I::Callback,
    /// Key of the identity.
    id: u32,
}

impl<I: Imports> Identity<I> {
    /// Asks the host for a fresh identity.
    pub(super) fn new(imports: &I) -> Self {
        let (id, handle) = imports.new_callback();
        Self { handle, id }
    }

    /// Keeps a closure for the identity until the host releases it. Dropping the identity
    /// without calling this retains nothing, which rolls back a rejected registration.
    pub(super) fn keep(self, closure: impl Stored) {
        let Self { handle, id } = self;
        insert(
            id,
            Entry {
                kind: closure.wrap(),
                _owner: Some(Box::new(handle)),
            },
        );
    }
}

/// An operation-scoped callback. The host calls the one-shot closure once: captures it keeps
/// drop when that call returns; captures moved into the returned future drop when the future
/// completes. The identity lasts until the operation ends: its table entry is released then,
/// which also drops a closure the host never ran, and the owner handle drops right after.
pub(super) struct Scoped<I: Imports> {
    /// The owner handle created by the host.
    pub(super) handle: I::Callback,
    /// Key of the identity.
    id: u32,
}

impl<I: Imports> Scoped<I> {
    /// Registers a closure that lives only for one operation.
    pub(super) fn new(imports: &I, closure: impl Stored) -> Self {
        let Identity { handle, id } = Identity::<I>::new(imports);
        insert(
            id,
            Entry {
                kind: closure.wrap(),
                _owner: None,
            },
        );
        Self { handle, id }
    }
}

impl<I: Imports> Drop for Scoped<I> {
    fn drop(&mut self) {
        release(self.id);
    }
}

/// Inserts an entry.
fn insert(id: u32, entry: Entry) {
    let replaced = TABLE.with_borrow_mut(|table| table.insert(id, entry));
    drop(replaced);
}

/// Removes an entry, then drops its closure and its owner handle after the table borrow
/// ended.
pub(crate) fn release(id: u32) {
    let removed = TABLE.with_borrow_mut(|table| table.remove(&id));
    drop(removed);
}

/// Finds the shared closure registered for a host identity.
///
/// # Errors
/// Returns a message naming the identity when no closure of that kind is registered.
pub(super) fn find<T: Shared>(id: u32) -> ExtensionResult<T> {
    TABLE
        .with_borrow(|table| table.get(&id).and_then(|entry| T::pick(&entry.kind)))
        .ok_or_else(|| format!("no callback registered for identity {id}"))
}

/// Takes the one-shot closure pending under a host identity.
///
/// # Errors
/// Returns a message naming the identity when no such closure is pending.
pub(super) fn take<T: Once>(id: u32) -> ExtensionResult<T> {
    TABLE
        .with_borrow_mut(|table| {
            table
                .get_mut(&id)
                .and_then(|entry| T::take(&mut entry.kind))
        })
        .ok_or_else(|| format!("no pending continuation for identity {id}"))
}
