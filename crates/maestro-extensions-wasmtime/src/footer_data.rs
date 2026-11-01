//! Footer metadata lent to a footer factory, read from its original native owner.
use crate::{
    guest_calls::{Callback, Handle, SynchronousKind},
    local_resources::{Lease, Message, Registry, effects::SubscriptionEndpoint},
    read_imports::{ReadEndpoint, add_read, resource},
};
use maestro_extensions::{ExtensionAPI, Local, footer_data_provider::ReadonlyFooterDataProvider};
use std::{
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Catchable native reply.
type Reply<T> = Sender<Result<T, String>>;

/// One read or subscription on the original footer metadata.
pub(crate) enum Request {
    /// Reads the current branch.
    Branch(Reply<Option<String>>),
    /// Reads the extension statuses in insertion order.
    Statuses(Reply<Vec<(String, String)>>),
    /// Reads the count of providers with available models.
    Providers(Reply<f64>),
    /// Adds an authored branch change listener under the original extension.
    OnBranchChange(
        Handle,
        crate::api_reads::Endpoint,
        Reply<SubscriptionEndpoint>,
    ),
}

impl Request {
    /// Answers from the original metadata outside all capability-table borrows.
    pub(crate) fn answer(self, footer: &Rc<dyn ReadonlyFooterDataProvider>, registry: &Registry) {
        match self {
            Self::Branch(reply) => drop(reply.send(Ok(footer.get_git_branch()))),
            Self::Statuses(reply) => {
                let statuses = footer.get_extension_statuses().iter().collect();
                drop(reply.send(Ok(statuses)));
            }
            Self::Providers(reply) => drop(reply.send(Ok(footer.get_available_provider_count()))),
            Self::OnBranchChange(listener, api, reply) => {
                let subscribed = registry
                    .facade(&api)
                    .and_then(|api| registry.branch_change(footer, listener, &api));
                drop(reply.send(subscribed));
            }
        }
    }

    /// Settles a released capability without leaving the Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Branch(reply) => drop(reply.send(Err(error))),
            Self::Statuses(reply) => drop(reply.send(Err(error))),
            Self::Providers(reply) => drop(reply.send(Err(error))),
            Self::OnBranchChange(.., reply) => drop(reply.send(Err(error))),
        }
    }
}

impl Registry {
    /// Adds the authored branch change listener to the original metadata and retains its
    /// remover; the listener runs under the original extension's callback frame.
    fn branch_change(
        &self,
        footer: &Rc<dyn ReadonlyFooterDataProvider>,
        listener: Handle,
        api: &ExtensionAPI<Local>,
    ) -> Result<SubscriptionEndpoint, String> {
        let (registry, events) = (self.clone(), api.events.clone());
        let remove = footer.on_branch_change(Rc::new(move || {
            let _ = events.invoke_callback(|emitter| {
                let emitter = registry.emitter(emitter.frame())?;
                let kind = SynchronousKind::BranchChange;
                listener.synchronous(&registry, kind, String::new(), emitter)
            });
        }));
        self.subscription_remover(Rc::from(remove))
    }
}

/// Store-safe ownership of the footer metadata lent to one factory.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl ReadEndpoint for Endpoint {
    type Request = Request;
    fn read<T>(
        &self,
        request: impl FnOnce(Sender<Result<T, String>>) -> Request,
    ) -> Result<T, String> {
        self.0
            .request(|id, reply| Message::Footer(id, request(reply)))
    }
    fn route<T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        request: impl FnOnce(Sender<Result<T, String>>) -> Request,
    ) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Footer(id, request(reply)))
    }
}

/// Installs the footer metadata resource and its reads.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "footer-data")?;
    add_read(
        host,
        "[method]footer-data.get-git-branch",
        Request::Branch,
        resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]footer-data.get-extension-statuses",
        Request::Statuses,
        resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]footer-data.get-available-provider-count",
        Request::Providers,
        resource::<T, Endpoint>,
    )?;
    host.func_wrap(
        "[method]footer-data.on-branch-change",
        |mut store: StoreContextMut<'_, T>,
         (footer, listener): (Resource<Endpoint>, Resource<Callback>)| {
            let listener = crate::callback_imports::retain(store.data_mut(), &listener)?;
            let api = store.data().api().clone();
            let endpoint = store.data_mut().ctx().table.get(&footer)?.clone();
            let result = endpoint.read(|reply| Request::OnBranchChange(listener, api, reply));
            let result = match result {
                Ok(subscription) => Ok(store.data_mut().ctx().table.push(subscription)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}
