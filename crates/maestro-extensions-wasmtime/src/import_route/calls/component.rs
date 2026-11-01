//! Nested retained component methods keep their original invocation frame.
use super::{Callback, LocalCall};
use crate::guest_calls::renderers::{ComponentCall, ComponentMethod};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Owned operands of one nested component render.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalRender {
    /// Authored identity of the retained component.
    target: u32,
    /// Actual caller viewport width.
    width: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested input or invalidation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalUnit {
    /// Authored identity of the retained component.
    target: u32,
    /// Literal input, or absence for invalidation.
    data: Option<String>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested key-release query.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalKey {
    /// Authored identity of the retained component.
    target: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of a synchronous focus assignment.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalFocus {
    /// Authored component identity.
    target: u32,
    /// New focus value.
    focused: bool,
    /// Original callback frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Marshals one retained component method.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &ComponentCall<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        ComponentCall::Render(owner, width, emitter, _) => LocalCall::Render(LocalRender {
            target: owner.id,
            width: *width,
            emitter: table.push(emitter.clone())?,
        }),
        ComponentCall::Unit(owner, ComponentMethod::SetExpanded(expanded), emitter, _) => {
            LocalCall::Expanded(super::LocalExpanded {
                target: owner.id,
                expanded: *expanded,
                emitter: table.push(emitter.clone())?,
            })
        }
        ComponentCall::Unit(owner, ComponentMethod::SetFocused(focused), emitter, _) => {
            LocalCall::SetFocused(LocalFocus {
                target: owner.id,
                focused: *focused,
                emitter: table.push(emitter.clone())?,
            })
        }
        ComponentCall::Unit(owner, method, emitter, _) => LocalCall::Unit(LocalUnit {
            target: owner.id,
            data: match method {
                ComponentMethod::Input(data) => Some(data.clone()),
                ComponentMethod::Invalidate
                | ComponentMethod::SetExpanded(_)
                | ComponentMethod::SetFocused(_) => None,
            },
            emitter: table.push(emitter.clone())?,
        }),
        ComponentCall::Focused(owner, emitter, _) => LocalCall::Focused(LocalKey {
            target: owner.id,
            emitter: table.push(emitter.clone())?,
        }),
        ComponentCall::WantsKeyRelease(owner, emitter, _) => LocalCall::Key(LocalKey {
            target: owner.id,
            emitter: table.push(emitter.clone())?,
        }),
    })
}
