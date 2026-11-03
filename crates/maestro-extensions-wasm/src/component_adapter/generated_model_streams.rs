//! Native assistant stream forwarding; only the independent decoded result is cached.
use super::{Generated, host};
use crate::component_adapter::{
    callbacks::{self, Identity},
    signals::{self, Binding},
};
use crate::types::finite::encode_value;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, EventStreamPort, ExtensionFuture,
    ExtensionResult, Model, OnPayload, OnResponse, PayloadHookInput, ProviderStreamOptions,
    SharedAssistantMessage, SimpleStreamOptions, StreamOptions,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Native assistant buffer and the originating guest controls.
struct Port {
    /// Original host resource shared by all observations and aliases.
    stream: Rc<host::AssistantStream>,
    /// Decoded final message owner; the native shared future remains the settlement owner.
    result: Rc<RefCell<Option<Rc<SharedAssistantMessage>>>>,
}
impl EventStreamPort<AssistantMessageEvent, SharedAssistantMessage> for Port {
    fn push(&self, event: AssistantMessageEvent) -> ExtensionResult<()> {
        self.stream.push(&encode_value(&event)?)
    }
    fn end(&self, value: Option<SharedAssistantMessage>) -> ExtensionResult<()> {
        self.stream
            .end(value.as_ref().map(encode_value).transpose()?.as_deref())
    }
    fn next(&self) -> ExtensionFuture<'static, Option<Rc<AssistantMessageEvent>>> {
        let stream = self.stream.clone();
        Box::pin(async move {
            stream
                .next()
                .await?
                .map(|data| super::super::session::decode(&data).map(Rc::new))
                .transpose()
        })
    }
    fn result(&self) -> ExtensionFuture<'static, Rc<SharedAssistantMessage>> {
        let (stream, result) = (self.stream.clone(), self.result.clone());
        Box::pin(async move {
            if let Some(value) = result.borrow().as_ref() {
                return Ok(value.clone());
            }
            let data = stream.result().await?;
            let decoded = super::super::session::decode(&data)?;
            Ok(result
                .borrow_mut()
                .get_or_insert_with(|| Rc::new(decoded))
                .clone())
        })
    }
}

/// Encode raw data separately from retained controls, without normalizing open fields.
pub(crate) fn raw(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> ExtensionResult<AssistantMessageEventStream> {
    let data = encode_value(&(
        model,
        context,
        options.as_ref().map(|o| (&o.common, &o.extra)),
        None::<&str>,
    ))?;
    let (common, objects) = options.map_or((None, None), |options| {
        (Some(options.common), options.objects)
    });
    invoke(false, &data, common.as_ref(), objects)
}
/// Encode the simple path's own policies without selecting similarly named raw fields.
pub(crate) fn simple(
    api: Option<&str>,
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> ExtensionResult<AssistantMessageEventStream> {
    let data = encode_value(&(
        model,
        context,
        options
            .as_ref()
            .map(|o| (&o.common, &o.reasoning, &o.thinking_budgets)),
        api,
    ))?;
    let common = options.map(|options| options.common);
    invoke(true, &data, common.as_ref(), None)
}
/// Retain each non-data operand under the existing callback identity owner.
fn invoke(
    simple: bool,
    data: &str,
    common: Option<&StreamOptions>,
    objects: Option<Rc<dyn std::any::Any>>,
) -> ExtensionResult<AssistantMessageEventStream> {
    let signal = common
        .and_then(|o| o.signal.as_ref())
        .map(|caller| Binding::new(&Generated, Some(caller)).map(Rc::new))
        .transpose()?;
    let payload = common.and_then(|o| o.on_payload.clone()).map(staged);
    let response = common.and_then(|o| o.on_response.clone()).map(staged);
    let retained: super::super::streams::Value = Rc::new((signal.clone(), objects));
    let objects = Some(staged(retained));
    let stream = host::model_stream(
        simple,
        data,
        &host::ModelControls {
            signal: signal.as_ref().and_then(|binding| binding.signal()),
            payload: payload.as_ref().map(|identity| &identity.handle),
            response: response.as_ref().map(|identity| &identity.handle),
            objects: objects.as_ref().map(|identity| &identity.handle),
        },
    );
    for identity in [payload, response, objects].into_iter().flatten() {
        identity.settle(stream.is_ok());
    }
    Ok(AssistantMessageEventStream::from_port(Rc::new(Port {
        stream: Rc::new(stream?),
        result: Rc::default(),
    })))
}
/// Stage without exposing callback storage while native code runs.
fn staged(value: impl callbacks::Stored) -> Identity<Generated> {
    let identity = Identity::new(&Generated);
    identity.stage(value);
    identity
}
/// Await the original hook outside callback-table borrows and preserve its selected signal.
pub(super) async fn hook(
    handler: u32,
    phase: host::ModelHook,
    data: String,
    model: String,
    signal: Option<host::AbortSignal>,
) -> ExtensionResult<host::PayloadSelection> {
    let model = Arc::new(super::super::session::decode(&model)?);
    let selected = match phase {
        host::ModelHook::Payload => {
            callbacks::find::<OnPayload>(handler)?(
                PayloadHookInput {
                    payload: super::super::session::decode(&data)?,
                    signal: signal.map(|signal| signals::signal(&Generated, signal)),
                },
                model,
            )
            .await?
        }
        host::ModelHook::Response => {
            callbacks::find::<OnResponse>(handler)?(super::super::session::decode(&data)?, model)
                .await?;
            return Ok(host::PayloadSelection {
                data: String::new(),
                signal: None,
                owner: None,
            });
        }
    };
    selected_signal(&selected)
}
/// Transfer selected cancellation together with its existing callback-table owner.
pub(super) fn selected_signal(
    selected: &PayloadHookInput,
) -> ExtensionResult<host::PayloadSelection> {
    let data = encode_value(&selected.payload)?;
    let binding = selected
        .signal
        .as_ref()
        .map(|signal| Binding::new(&Generated, Some(signal)))
        .transpose()?;
    let signal = binding
        .as_ref()
        .and_then(|binding| binding.signal())
        .map(host::AbortSignal::retain);
    let owner = binding.map(|binding| {
        let value: super::super::streams::Value = Rc::new(binding);
        Identity::new(&Generated).transfer(value)
    });
    Ok(host::PayloadSelection {
        data,
        signal,
        owner,
    })
}
