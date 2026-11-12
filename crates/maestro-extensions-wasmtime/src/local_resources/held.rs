//! Typed access to one retained capability for the pump's generic call kinds.
use super::{Capability, Lease, Message, Registry, Work};
use crate::context_reads::Owner;
use maestro_extensions::{ExtensionCommandContext, ModelRegistry, SharedHandle};
use std::{future::Future, sync::mpsc};

/// One kind of retained capability, with the text its callers receive once it has ended.
pub(crate) trait Held {
    /// What the capability lends to a call on the local owner.
    type Target: ?Sized;
    /// The failure text of a call whose capability has ended or is another kind.
    const ENDED: &'static str;
    /// Lends the capability when the entry holds this kind.
    fn lend(capability: &Capability) -> Option<&Self::Target>;
}

/// Declares one kind of retained capability: what it lends, its ended text and the entry that
/// holds it.
macro_rules! held {
    ($(#[$doc:meta])* $name:ident, $target:ty, $ended:literal, $pattern:pat => $lent:expr) => {
        $(#[$doc])*
        pub(crate) struct $name;

        impl Held for $name {
            type Target = $target;
            const ENDED: &'static str = $ended;
            fn lend(capability: &Capability) -> Option<&Self::Target> {
                match capability {
                    $pattern => Some($lent),
                    _ => None,
                }
            }
        }
    };
}

held!(
    /// The independently retained model catalog.
    Catalog,
    SharedHandle<ModelRegistry>,
    "extension catalog resource ended",
    Capability::Catalog(catalog) => catalog
);
held!(
    /// The retained context that carries the session-replacing operations.
    SessionCommands,
    ExtensionCommandContext,
    "extension command context resource ended",
    Capability::Context(Owner::Command(context)) => context
);
held!(
    /// The live theme the renderer lent.
    Theme,
    maestro_extensions::LiveTheme,
    "extension theme resource ended",
    Capability::Theme(theme) => theme
);

held!(
    /// A node of one captured session tree.
    Node,
    crate::tree_reads::Owner,
    "extension tree node resource ended",
    Capability::Node(node) => node
);

held!(
    /// The retained read-only view of a session.
    History,
    std::rc::Rc<std::cell::RefCell<dyn maestro_extensions::ReadonlySessionManager>>,
    "extension history resource ended",
    Capability::History(history) => history
);

held!(
    /// The writable session the setup callback received.
    Writer,
    std::rc::Rc<std::cell::RefCell<maestro_extensions::SessionManager>>,
    "extension writable session resource ended",
    Capability::Writer(session) => session
);

held!(
    /// The cancellation a context, a signal creation or a provider call lent.
    Signal,
    maestro_extensions::Cancellation,
    "extension signal resource ended",
    Capability::Signal(signal) => signal
);

held!(
    /// The footer metadata lent to a footer factory.
    Footer,
    std::rc::Rc<dyn maestro_extensions::footer::ReadonlyFooterDataProvider>,
    "extension footer resource ended",
    Capability::Footer(footer) => footer
);

held!(
    /// The progress observer a tool invocation supplied.
    Update,
    maestro_extensions::AgentToolUpdateCallback<serde_json::Value, maestro_extensions::Local>,
    "extension tool-update resource ended",
    Capability::ToolUpdate(update) => update
);

held!(
    /// The redraw capability of one tool row.
    Invalidator,
    std::rc::Rc<dyn Fn()>,
    "extension tool-invalidator resource ended",
    Capability::Invalidate(invalidate) => invalidate
);

/// An explicit remover of a registered observer or listener.
pub(crate) trait Remove {
    /// Removes the registration, which a drop of its resource leaves in place.
    fn remove(&self) -> Result<(), String>;
}

impl Remove for maestro_extensions::Subscription<maestro_extensions::Local> {
    fn remove(&self) -> Result<(), String> {
        self.unsubscribe();
        Ok(())
    }
}

impl Remove for std::rc::Rc<dyn Fn()> {
    fn remove(&self) -> Result<(), String> {
        self();
        Ok(())
    }
}

impl Remove for std::rc::Rc<dyn Fn() -> Result<(), maestro_extensions::EventError>> {
    fn remove(&self) -> Result<(), String> {
        self().map_err(|error| error.to_string())
    }
}

/// The remover behind a subscription resource: an event bus subscription, an abort observer
/// or a terminal input listener.
pub(crate) struct Remover;

impl Held for Remover {
    type Target = dyn Remove;
    const ENDED: &'static str = "extension subscription resource ended";
    fn lend(capability: &Capability) -> Option<&Self::Target> {
        match capability {
            Capability::Subscription(remover) => Some(remover),
            Capability::SignalSubscription(remover) => Some(remover),
            Capability::UiSubscription(remover) => Some(remover),
            _ => None,
        }
    }
}

held!(
    /// The data stream of one command execution.
    OutputFrame,
    std::rc::Rc<super::bash_output::Frame>,
    "extension bash-output resource ended",
    Capability::BashOutput(frame) => frame
);

held!(
    /// The close operation of a custom component.
    Close,
    maestro_extensions::types::CustomClose,
    "extension custom close resource ended",
    Capability::CustomClose(close) => close
);

held!(
    /// A host style the extension may apply to text.
    LentStyle,
    maestro_extensions::presentation::BorderColor,
    "extension style resource ended",
    Capability::Style(style) => style
);

held!(
    /// The host's code highlighting.
    LentHighlighter,
    std::rc::Rc<crate::guest_calls::paints::HighlightFn>,
    "extension style resource ended",
    Capability::Highlighter(highlight) => highlight
);

held!(
    /// A settings row style the host lent.
    LentRowStyle,
    std::rc::Rc<crate::widget_lists::RowStyleFn>,
    "extension style resource ended",
    Capability::RowStyle(style) => style
);

held!(
    /// The closer of an open settings submenu.
    SubmenuCloser,
    std::rc::Rc<dyn Fn(Option<String>)>,
    "extension style resource ended",
    Capability::SubmenuDone(done) => done
);

held!(
    /// A native text callback lent to an authored editor.
    LentText,
    maestro_extensions::presentation::TextCallback,
    "extension style resource ended",
    Capability::Text(callback) => callback
);

/// Builds the inline call kind: the call runs on the local owner and replies on its channel.
fn run_message<H: Held, T: Send + 'static>(
    id: u64,
    reply: mpsc::Sender<Result<T, String>>,
    call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
) -> Message {
    Message::Run(
        id,
        Box::new(move |capability, registry| {
            let result = match capability.and_then(H::lend) {
                Some(held) => call(held, registry),
                None => Err(H::ENDED.to_owned()),
            };
            let _ = reply.send(result);
        }),
    )
}

/// Settles the caller's reply with the started work's result, or with the failure that kept the
/// work from starting.
fn settle<T: 'static, F: Future<Output = Result<T, String>> + 'static>(
    started: Result<F, String>,
    reply: tokio::sync::oneshot::Sender<Result<T, String>>,
) -> Work {
    Box::pin(async move {
        let result = match started {
            Ok(work) => work.await,
            Err(error) => Err(error),
        };
        let _ = reply.send(result);
    })
}

impl Lease {
    /// Runs `call` on the local owner against the held capability while the Store thread
    /// waits for the result.
    pub(crate) fn run<H: Held, T: Send + 'static>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        self.request(|id, reply| run_message::<H, T>(id, reply, call))
    }

    /// Runs `call` like [`Lease::run`], for a native operation that may call back into this
    /// instance; the guest drives the returned route until the result arrives.
    pub(crate) fn run_routed<H: Held, T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
    ) -> crate::import_route::Route {
        self.route(|id, reply| run_message::<H, T>(id, reply, call))
    }

    /// Starts the work `call` selects on the local owner and awaits its result without
    /// blocking the Store; the lease's admission stays held until the work settles.
    pub(crate) async fn admit<H, T, F>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<F, String> + Send + 'static,
    ) -> Result<T, String>
    where
        H: Held,
        T: Send + 'static,
        F: Future<Output = Result<T, String>> + 'static,
    {
        self.request_async(|id, reply, admission| {
            Message::Admit(
                id,
                Box::new(move |capability, registry| -> Work {
                    let started = match capability.and_then(H::lend) {
                        Some(held) => call(held, registry),
                        None => Err(H::ENDED.to_owned()),
                    };
                    settle(started, reply)
                }),
                admission,
            )
        })
        .await
    }
}
