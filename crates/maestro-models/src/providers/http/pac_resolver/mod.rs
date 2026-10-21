//! Retained script execution outside the asynchronous executor.

mod date_range;
mod dns_domain_is;
mod dns_domain_levels;
mod dns_resolve;
mod ip;
mod is_in_net;
mod is_plain_host_name;
mod is_resolvable;
mod local_host_or_domain_is;
mod my_ip_address;
mod sh_exp_match;
mod time_range;
mod util;
mod weekday_range;

use std::sync::Arc;

use boa_engine::builtins::promise::PromiseState;
use boa_engine::object::builtins::JsPromise;
use boa_engine::{Context, JsNativeError, JsResult, JsValue, NativeFunction, Source, js_string};
use tokio::sync::{mpsc, oneshot};

use super::get_uri::Failure;

/// One invocation of the retained callable.
struct Request {
    /// Identity shared only with this invocation's cancellation notification.
    token: Arc<()>,
    /// Complete destination URL.
    url: String,
    /// Completion of the admitted invocation.
    reply: oneshot::Sender<Result<String, String>>,
}

/// Arrivals that admit work or release an abandoned result.
enum Command {
    /// Invoke the retained callable.
    Resolve(Request),
    /// Release a result whose requester was dropped.
    Cancel(Arc<()>),
}

/// Cancellation follows the requester's future, including a drop before settlement.
struct Invocation {
    /// Arrival channel for the existing interpreter worker.
    commands: mpsc::UnboundedSender<Command>,
    /// Identity of this invocation, independent of other callers.
    token: Arc<()>,
}

impl Drop for Invocation {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Cancel(self.token.clone()));
    }
}

/// One authored result that may settle on a later arrival.
struct Pending {
    /// Requester and its cancellation identity.
    request: Request,
    /// Engine-owned promise, including assimilated thenables.
    promise: JsPromise,
}

/// Owned command channel; dropping it lets the interpreter worker finish.
pub(super) struct Resolver {
    /// Serial invocations on the same context.
    commands: Option<mpsc::UnboundedSender<Command>>,
    /// Blocking interpreter lifetime; queued startup is aborted when the owner is released.
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl Resolver {
    /// Compile once before admitting calls against this context.
    pub(super) async fn new(source: String) -> Result<Self, Failure> {
        let (commands, requests) = mpsc::unbounded_channel();
        let (ready, initialized) = oneshot::channel();
        let runtime = tokio::runtime::Handle::current();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let worker = tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatch, || {
                execute(&source, runtime, requests, ready);
            });
        });
        let resolver = Self {
            commands: Some(commands),
            worker: Some(worker),
        };
        initialized.await?.map_err(std::io::Error::other)?;
        Ok(resolver)
    }

    /// Evaluate the actual callable with the complete URL before connection preparation.
    pub(super) async fn resolve(&self, url: &str) -> Result<String, Failure> {
        let (reply, result) = oneshot::channel();
        let commands = self
            .commands
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Resolver is closed"))?;
        let invocation = Invocation {
            commands: commands.clone(),
            token: Arc::new(()),
        };
        commands.send(Command::Resolve(Request {
            token: invocation.token.clone(),
            url: url.into(),
            reply,
        }))?;
        result
            .await?
            .map_err(|error| std::io::Error::other(error).into())
    }

    /// Close the command channel and wait for admitted invocations to settle.
    #[cfg(test)]
    pub(super) async fn close(mut self) -> Result<(), Failure> {
        self.commands.take();
        if let Some(worker) = self.worker.take() {
            worker.await?;
        }
        Ok(())
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

/// Initialize and drive one context exclusively on its blocking worker.
fn execute(
    source: &str,
    runtime: tokio::runtime::Handle,
    mut requests: mpsc::UnboundedReceiver<Command>,
    ready: oneshot::Sender<Result<(), String>>,
) {
    let mut context = Context::default();
    context.insert_data(util::Host(runtime));
    let initialized = initialize(&mut context, source).map_err(|error| error.to_string());
    let success = initialized.is_ok();
    if ready.send(initialized).is_err() || !success {
        return;
    }
    let mut pending = Vec::new();
    while let Some(command) = requests.blocking_recv() {
        match command {
            Command::Cancel(token) => {
                pending.retain(|item: &Pending| !Arc::ptr_eq(&item.request.token, &token));
            }
            Command::Resolve(request) => admit(&mut context, &mut pending, request),
        }
    }
}

/// Admit one live requester before driving jobs that its invocation may settle.
fn admit(context: &mut Context, pending: &mut Vec<Pending>, request: Request) {
    if request.reply.is_closed() {
        return;
    }
    match evaluate(context, &request.url) {
        Ok(promise) => pending.push(Pending { request, promise }),
        Err(error) => {
            let _ = request.reply.send(Err(error.to_string()));
        }
    }
    settle(context, pending);
}

/// Install native helpers and compile the authored program without rewriting it.
fn initialize(context: &mut Context, source: &str) -> JsResult<()> {
    let helpers: &[(
        &str,
        usize,
        boa_engine::native_function::NativeFunctionPointer,
    )] = &[
        ("alert", 0, alert),
        ("myIpAddress", 0, my_ip_address::my_ip_address),
        ("dateRange", 0, date_range::date_range),
        ("weekdayRange", 3, weekday_range::weekday_range),
        ("timeRange", 0, time_range::time_range),
        ("isInNet", 3, |_, args, context| {
            Ok(is_in_net::is_in_net(args, context))
        }),
        ("isResolvable", 1, is_resolvable::is_resolvable),
        ("dnsResolve", 1, dns_resolve::dns_resolve),
        ("shExpMatch", 2, sh_exp_match::sh_exp_match),
        ("dnsDomainIs", 2, dns_domain_is::dns_domain_is),
        ("dnsDomainLevels", 1, dns_domain_levels::dns_domain_levels),
        ("isPlainHostName", 1, is_plain_host_name::is_plain_host_name),
        (
            "localHostOrDomainIs",
            2,
            local_host_or_domain_is::local_host_or_domain_is,
        ),
    ];
    for (name, arity, function) in helpers {
        context.register_global_builtin_callable(
            js_string!(*name),
            *arity,
            NativeFunction::from_fn_ptr(*function),
        )?;
    }
    context.eval(Source::from_bytes(source))?;
    Ok(())
}

/// Emit an authored alert without printing or changing its message.
fn alert(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let message = if args.first().is_none_or(JsValue::is_undefined) {
        String::new()
    } else {
        util::string(args, 0, context)?
    };
    tracing::info!(target: "maestro_models::pac", message = message.as_str());
    Ok(JsValue::undefined())
}

/// Invoke a retained callable, applying the interpreter's own truth and string conversions.
fn evaluate(context: &mut Context, text: &str) -> JsResult<JsPromise> {
    let url = url::Url::parse(text)
        .map_err(|error| JsNativeError::typ().with_message(error.to_string()))?;
    let host = url
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| JsNativeError::typ().with_message("Could not determine `host`"))?;
    let callable = context
        .global_object()
        .get(js_string!("FindProxyForURL"), context)?
        .as_callable()
        .ok_or_else(|| JsNativeError::typ().with_message("FindProxyForURL is not callable"))?;
    let value = callable.call(
        &JsValue::undefined(),
        &[js_string!(text).into(), js_string!(host).into()],
        context,
    )?;
    JsPromise::resolve(value, context)
}

/// Run queued jobs only after a callable can have scheduled or settled work.
fn settle(context: &mut Context, pending: &mut Vec<Pending>) {
    if let Err(error) = context.run_jobs() {
        for item in pending.drain(..) {
            let _ = item.request.reply.send(Err(error.to_string()));
        }
        return;
    }
    let mut index = 0;
    while index < pending.len() {
        let result = match pending[index].promise.state() {
            PromiseState::Pending if !pending[index].request.reply.is_closed() => {
                index += 1;
                continue;
            }
            PromiseState::Pending => None,
            PromiseState::Fulfilled(value) => Some(convert(&value, context)),
            PromiseState::Rejected(reason) => Some(Err(boa_engine::JsError::from_opaque(reason))),
        };
        let item = pending.swap_remove(index);
        if let Some(result) = result {
            let _ = item
                .request
                .reply
                .send(result.map_err(|error| error.to_string()));
        }
    }
}

/// Convert only the settled value, preserving falsey DIRECT and native string semantics.
fn convert(value: &JsValue, context: &mut Context) -> JsResult<String> {
    if !value.to_boolean() {
        return Ok("DIRECT".into());
    }
    value
        .to_string(context)
        .map(|value| value.to_std_string_escaped())
}

#[cfg(test)]
mod tests;
