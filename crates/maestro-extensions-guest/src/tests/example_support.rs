//! Support shared by the controlled-host tests that start a factory and check its registrations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::controlled::Controlled;
use crate::component_adapter::{Capabilities, Exports, release};
use crate::loader::Extension;

/// Starts a factory on a fresh controlled host.
pub async fn start<E: Extension>() -> Result<(Controlled, Exports<Controlled>), String> {
    let host = Controlled::default();
    let exports = Exports::new(host.clone());
    exports.start::<E>().await?;
    Ok((host, exports))
}

/// Delivers one lifecycle event through its registered handler and returns the handler's
/// failure, if any.
pub fn event(host: &Controlled, exports: &Exports<Controlled>, name: &str) -> Result<(), String> {
    let key = host.observed().callback(&format!("event {name}"))?;
    let event = super::documents::document(name).to_string();
    let outcome = super::block_on(exports.invoke_event(
        key,
        event,
        Capabilities {
            ctx: host.context("/work"),
            signal: None,
        },
    ))?;
    let answer = super::documents::Answer::of(super::guest_family::delivery(outcome))?;
    match answer.ended {
        super::documents::Ended::Returned(None) => Ok(()),
        super::documents::Ended::Failed(error) => Err(error),
        other @ super::documents::Ended::Returned(_) => {
            Err(format!("unexpected event answer: {other:?}"))
        }
    }
}

/// Releases list handlers and every registration in arrival order, asserting the host admitted
/// exactly `expected` and kept nothing lent.
pub fn release_all(host: &Controlled, expected: &[impl AsRef<str>]) {
    let handlers = host.observe().ui.lists.take_handlers();
    for key in handlers {
        release(key);
    }
    let expected: Vec<&str> = expected.iter().map(AsRef::as_ref).collect();
    let names = released(host);
    assert_eq!(names, expected, "the registrations the host received");
    assert_eq!(host.observed().live, 0, "the host lent nothing it kept");
}

/// Releases every registration of the controlled host in the order it received them and
/// returns their names.
pub fn released(host: &Controlled) -> Vec<String> {
    let mut names = Vec::new();
    loop {
        let next = host.observe().next_to_release();
        let Some((name, key)) = next else { break };
        release(key);
        names.push(name);
    }
    names
}
