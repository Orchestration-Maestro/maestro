//! Native interaction capabilities retain their original callbacks and live cancellation.
use super::{Generated, host, route};
use crate::{ExtensionFuture, ExtensionResult, OAuthLoginCallbacks};
use std::rc::Rc;

/// Projects native callback presence without starting any optional callback.
pub(super) fn interaction(resource: host::OauthInteraction) -> Rc<OAuthLoginCallbacks> {
    let options = resource.options();
    let signal = resource
        .signal()
        .map(|signal| super::super::signals::signal(&Generated, signal));
    let resource = Rc::new(resource);
    Rc::new(OAuthLoginCallbacks {
        on_auth: {
            let resource = resource.clone();
            Rc::new(move |info| {
                notify(
                    &resource,
                    host::OauthCallback::Auth,
                    &crate::types::finite::encode_value(&info)?,
                )
            })
        },
        on_prompt: {
            let resource = resource.clone();
            Rc::new(move |prompt| {
                required(input(
                    resource.clone(),
                    host::OauthCallback::Prompt,
                    crate::types::finite::encode_value(&prompt),
                ))
            })
        },
        on_progress: options.0.then(|| {
            let resource = resource.clone();
            Rc::new(move |message: &str| notify(&resource, host::OauthCallback::Progress, message))
                as crate::OAuthProgressCallback
        }),
        on_manual_code_input: options.1.then(|| {
            let resource = resource.clone();
            Rc::new(move || {
                required(input(
                    resource.clone(),
                    host::OauthCallback::Manual,
                    Ok(String::new()),
                ))
            }) as crate::OAuthManualCodeCallback
        }),
        on_select: options.2.then(|| {
            Rc::new(move |prompt| {
                input(
                    resource.clone(),
                    host::OauthCallback::Select,
                    crate::types::finite::encode_value(&prompt),
                )
            }) as crate::OAuthSelectCallback
        }),
        signal,
    })
}
/// Finishes synchronous delivery, including same-instance reentry, before returning.
fn notify(
    resource: &host::OauthInteraction,
    phase: host::OauthCallback,
    data: &str,
) -> ExtensionResult<()> {
    route::drive(&resource.notify(phase, data))
}
/// Retains the original native callback independently of its provider or initiating facade.
fn input(
    resource: Rc<host::OauthInteraction>,
    phase: host::OauthCallback,
    data: ExtensionResult<String>,
) -> ExtensionFuture<'static, Option<String>> {
    Box::pin(async move { resource.input(phase, data?).await })
}
/// Required input keeps an absent native callback distinct from an empty answer.
fn required(input: ExtensionFuture<'static, Option<String>>) -> ExtensionFuture<'static, String> {
    Box::pin(async move {
        input
            .await?
            .ok_or_else(|| "missing OAuth text response".into())
    })
}

/// Invokes the original authored interaction with its retained native capability.
pub(super) async fn invoke(call: host::LocalOauthCallback) -> ExtensionResult<Option<String>> {
    super::super::oauth::invoke(
        call.target,
        call.phase,
        &call.data,
        call.interaction.map(interaction),
    )
    .await
}
