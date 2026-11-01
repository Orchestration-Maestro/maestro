//! Cancellation listeners on the supplied context signal.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortSignal, ExtensionAPI, ExtensionCommandContext, ExtensionResult, FlagOptions, FlagType,
    Subscription,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

thread_local! {
    /// Subscriptions and their signals the component keeps, by label.
    static KEPT: RefCell<HashMap<String, (AbortSignal, Subscription)>> = RefCell::default();
}

/// Registers a flag named after its label when the listener's captures are released.
pub(super) struct Released(pub(super) ExtensionAPI, pub(super) String);
impl Drop for Released {
    fn drop(&mut self) {
        let options = FlagOptions {
            description: None,
            type_: FlagType::Boolean,
            default: None,
        };
        let _ = self
            .0
            .register_flag(&format!("released-{}", self.1), options);
    }
}

/// Runs a labeled subscription, failing listener, explicit removal or remover drop.
pub(super) fn command(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let (action, label) = request.split_once(' ').ok_or("missing signal label")?;
    if action == "unsubscribe" || action == "drop" {
        let kept = KEPT.with_borrow_mut(|kept| kept.remove(label));
        let (_, subscription) = kept.ok_or("unknown signal label")?;
        if action == "unsubscribe" {
            subscription.unsubscribe()?;
            return Ok(format!("unsubscribed {label}"));
        }
        return Ok(format!("dropped {label}"));
    }
    let signal = context.signal()?.ok_or("missing signal")?;
    let capture = Released(api.clone(), label.to_owned());
    let (failing, label) = (action == "fail", label.to_owned());
    let observed = label.clone();
    let subscription = signal.subscribe(Rc::new(move |emitter| {
        let _ = &capture;
        emitter.emit("abort-observed", serde_json::json!(observed))?;
        if failing {
            return Err(format!("abort listener {observed} failed"));
        }
        Ok(())
    }))?;
    KEPT.with_borrow_mut(|kept| kept.insert(label.clone(), (signal, subscription)));
    Ok(format!("subscribed {label}"))
}
