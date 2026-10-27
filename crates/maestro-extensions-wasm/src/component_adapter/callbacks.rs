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
        Renderer(crate::MessageRenderer),
        CompactionComplete(crate::CompactionComplete),
        CompactionError(crate::CompactionError),
        Listener(crate::EventListener),
        Watch(super::io::WatchCallback),
        Command(CommandHandler),
        Completion(crate::ArgumentCompletions),
        Shortcut(crate::ShortcutHandler),
        Prepare(crate::PrepareArguments),
        Tool(crate::ToolExecute),
        Component(crate::Component),
        Bash(crate::BashOperations),
        Read(crate::ReadOperations),
        Write(crate::WriteOperations),
        Edit(crate::EditOperations),
        Find(crate::FindOperations),
        Ls(crate::LsOperations),
        Grep(crate::GrepOperations),
        Schema(crate::schema_code::Handler),
        CredentialBackend(std::rc::Rc<dyn crate::AuthStorageBackend>),
        CredentialFallback(crate::FallbackResolver),
        OAuthCallbacks(std::rc::Rc<super::oauth::Interaction>),
        OAuthProvider(std::rc::Rc<dyn crate::OAuthProviderInterface>),
        TerminalInput(crate::TerminalInputHandler),
        ComponentFactory(crate::ComponentFactory),
        CustomFactory(crate::CustomComponentFactory),
        ComponentExpansion(crate::ComponentExpansionHandler),
        ToolRenderCall(crate::ToolRenderCall),
        ToolRenderResult(crate::ToolRenderResult),
        ToolRow(std::rc::Rc<crate::types::ToolRow>),
        FooterFactory(crate::FooterFactory),
        BranchChange(crate::BranchChangeListener),
        Style(crate::Style),
        Highlight(crate::Highlight),
        Widget(std::rc::Rc<super::lists::WidgetHandler>),
        EditorFactory(crate::EditorFactory),
        Editor(std::rc::Rc<dyn crate::EditorComponent>),
        ProviderFactory(crate::AutocompleteProviderFactory),
        Provider(std::rc::Rc<dyn crate::AutocompleteProvider>),
        StreamValue(super::streams::Value),
        ProviderStream(crate::StreamFunction<crate::StreamOptionsInput>),
        AssistantStream(crate::AssistantMessageEventStream),
        ModelPayload(crate::OnPayload),
        ModelResponse(crate::OnResponse),
        StreamClassifier(super::streams::Classifier),
        StreamExtractor(super::streams::Extractor),
    }
    once {
        Abort(crate::AbortListener),
        CustomFuture(crate::ExtensionFuture<'static, crate::ExtensionComponent>),
        WithSession(WithSession),
        Setup(crate::SetupSession),
        Tail(crate::ExtensionFuture<'static, ()>),
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
    /// Key of the identity.
    pub(super) fn id(&self) -> u32 {
        self.id
    }

    /// Asks the host for a fresh identity.
    pub(super) fn new(imports: &I) -> Self {
        let (id, handle) = imports.new_callback();
        Self { handle, id }
    }

    /// Hands the identity handle to the host while the object stays in this table.
    pub(super) fn transfer(self, object: impl Stored) -> I::Callback {
        let Self { handle, id } = self;
        insert(
            id,
            Entry {
                kind: object.wrap(),
                _owner: None,
            },
        );
        handle
    }

    /// Makes a closure callable under this identity while the request announcing it is
    /// still with the host, which may call it before answering.
    pub(super) fn stage(&self, closure: impl Stored) {
        insert(
            self.id,
            Entry {
                kind: closure.wrap(),
                _owner: None,
            },
        );
    }

    /// Keeps a staged closure until the host releases it, or withdraws it when the host
    /// rejected its request.
    pub(super) fn settle(self, accepted: bool) {
        let Self { handle, id } = self;
        if !accepted {
            release(id);
            return;
        }
        let staged = TABLE.with_borrow_mut(|table| table.remove(&id));
        if let Some(Entry { kind, .. }) = staged {
            insert(
                id,
                Entry {
                    kind,
                    _owner: Some(Box::new(handle)),
                },
            );
        }
    }

    /// Adopt a fresh host-supplied identity for a retained originating callback.
    pub(super) fn from_handle(imports: &I, handle: I::Callback) -> Self {
        let id = imports.callback_id(&handle);
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
    /// Adopt an already retained local identity and release it on every exit path.
    pub(super) fn from_handle(imports: &I, handle: I::Callback) -> Self {
        let id = imports.callback_id(&handle);
        Self { handle, id }
    }

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
