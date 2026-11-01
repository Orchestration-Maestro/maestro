//! Original provider and returned stream invocation over retained native capabilities.
use super::{Generated, host};
use crate::component_adapter::{callbacks, signals};
use crate::types::finite::encode_value;
use crate::{ExtensionResult, PayloadHookInput, StreamOptions, StreamOptionsInput};
use std::{rc::Rc, sync::Arc};

/// Decode only ordinary option data, then attach its retained native controls.
fn options(
    data: &str,
    input: host::ProviderInput,
) -> ExtensionResult<(crate::Model, crate::Context, Option<StreamOptionsInput>)> {
    let (model, context, options): (_, _, Option<(bool, StreamOptions, _, _, _)>) =
        super::super::session::decode(data)?;
    let controls = Rc::new(input.controls);
    let signal = input
        .signal
        .map(|signal| signals::signal(&Generated, signal));
    let options = options.map(|(simple, mut common, extra, reasoning, thinking_budgets)| {
        common.signal = signal;
        common.on_payload = input.payload.then(|| payload_hook(controls.clone()));
        common.on_response = input.response.then(|| response_hook(controls.clone()));
        if simple {
            StreamOptionsInput::Simple(crate::SimpleStreamOptions {
                common,
                reasoning,
                thinking_budgets,
                tool_choice: None,
            })
        } else {
            StreamOptionsInput::Raw(crate::ProviderStreamOptions {
                common,
                extra,
                objects: Some(controls.clone()),
            })
        }
    });
    Ok((model, context, options))
}

/// Await the native selector, retaining arbitrary selected guest bindings on its owner.
async fn payload(
    controls: &host::ProviderControls,
    input: PayloadHookInput,
    model: Arc<crate::Model>,
) -> ExtensionResult<PayloadHookInput> {
    let selection = super::model_streams::selected_signal(&input)?;
    let selected = controls
        .invoke(
            host::ModelHook::Payload,
            selection.data,
            encode_value(model.as_ref())?,
            selection.signal.as_ref(),
            selection.owner.as_ref(),
        )
        .await?;
    Ok(PayloadHookInput {
        payload: super::super::session::decode(&selected.data)?,
        signal: selected
            .signal
            .map(|signal| signals::signal(&Generated, signal)),
    })
}

/// Run setup synchronously and transfer the actual authored stream, not a draining future.
pub(super) fn invoke(
    target: u32,
    data: &str,
    input: host::ProviderInput,
) -> ExtensionResult<host::Callback> {
    let (model, context, options) = options(data, input)?;
    let stream = callbacks::find::<crate::StreamFunction<StreamOptionsInput>>(target)?(
        model, context, options,
    )?;
    Ok(callbacks::Identity::new(&Generated).transfer(stream))
}

/// Operate directly on the retained stream; independent results never drain events.
pub(super) async fn assistant(
    target: u32,
    operation: host::AssistantOperation,
    data: Option<String>,
    signal: Option<host::AbortSignal>,
) -> ExtensionResult<Option<String>> {
    let stream = callbacks::find::<crate::AssistantMessageEventStream>(target)?;
    match operation {
        host::AssistantOperation::Push => {
            stream.push(super::super::session::decode(
                &data.ok_or("missing assistant event")?,
            )?)?;
            Ok(None)
        }
        host::AssistantOperation::End => {
            stream.end(
                data.as_deref()
                    .map(super::super::session::decode)
                    .transpose()?,
            )?;
            Ok(None)
        }
        host::AssistantOperation::Next => {
            observe(
                signal,
                Box::pin(async move {
                    stream
                        .next()
                        .await?
                        .map(|event| encode_value(event.as_ref()))
                        .transpose()
                }),
            )
            .await
        }
        host::AssistantOperation::Result => {
            observe(
                signal,
                Box::pin(async move {
                    stream
                        .result()
                        .await
                        .and_then(|message| encode_value(&message))
                        .map(Some)
                }),
            )
            .await
        }
    }
}

/// Remove only an abandoned observation's original waiter, never its producer.
async fn observe(
    signal: Option<host::AbortSignal>,
    mut future: crate::ExtensionFuture<'_, Option<String>>,
) -> ExtensionResult<Option<String>> {
    use std::{
        cell::{Cell, RefCell},
        task::{Poll, Waker},
    };
    let Some(signal) = signal else {
        return future.await;
    };
    let signal = signals::signal(&Generated, signal);
    let cancelled = Rc::new(Cell::new(false));
    let waker: Rc<RefCell<Option<Waker>>> = Rc::default();
    let (flag, wake) = (cancelled.clone(), waker.clone());
    let _listening = signals::Listening(Some(signal.subscribe(Rc::new(move |_| {
        flag.set(true);
        if let Some(waker) = wake.borrow_mut().take() {
            waker.wake();
        }
        Ok(())
    }))?));
    cancelled.set(signal.aborted());
    std::future::poll_fn(|context| {
        if cancelled.get() {
            return Poll::Ready(Err("extension stream observation dropped".into()));
        }
        *waker.borrow_mut() = Some(context.waker().clone());
        future.as_mut().poll(context)
    })
    .await
}

/// Retain the original payload capability in the guest's ordinary hook representation.
fn payload_hook(controls: Rc<host::ProviderControls>) -> crate::OnPayload {
    Rc::new(move |input, model| {
        let controls = controls.clone();
        Box::pin(async move { payload(&controls, input, model).await })
    })
}
/// Retain the original response capability without adding protocol policy.
fn response_hook(controls: Rc<host::ProviderControls>) -> crate::OnResponse {
    Rc::new(move |response, model| {
        let controls = controls.clone();
        Box::pin(async move {
            controls
                .invoke(
                    host::ModelHook::Response,
                    encode_value(&response)?,
                    encode_value(model.as_ref())?,
                    None,
                    None,
                )
                .await?;
            Ok(())
        })
    })
}
