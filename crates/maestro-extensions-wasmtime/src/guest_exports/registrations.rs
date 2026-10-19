//! Typed command, completion and shortcut exports on the active Store accessor.
use super::{Session, arguments, session};
use crate::{
    context_imports::{CommandContext, Context},
    guest_calls::{Callback, RegisteredCall},
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lift, Resource, TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

/// Actual command export with its original owned command resource.
type Command =
    TypedFunc<(Resource<Callback>, String, Resource<CommandContext>), (Result<(), String>,)>;
/// Actual completion export, preserving absent and present-empty lists.
type Completion = TypedFunc<(Resource<Callback>, String), (Result<Option<Vec<Item>>, String>,)>;

/// Owned canonical completion item, converted directly to its native domain record.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Item {
    /// Literal inserted text.
    value: String,
    /// Literal displayed label.
    label: String,
    /// Independently optional authored description.
    description: Option<String>,
}

impl From<Item> for maestro_extensions::AutocompleteItem {
    fn from(item: Item) -> Self {
        Self {
            value: item.value,
            label: item.label,
            description: item.description,
        }
    }
}

/// Real exports for native command and shortcut registrations.
pub(super) struct Exports {
    /// Actual command entry point.
    command: Command,
    /// Actual argument-completion entry point.
    completion: Completion,
    /// Actual ordinary-context shortcut entry point.
    shortcut: Session<Context>,
}

impl Exports {
    /// Resolves required exports from the same canonical component instance.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        let mut index = |name| {
            instance
                .get_export_index(&mut *store, Some(guest), name)
                .ok_or_else(|| wasmtime::format_err!("missing extension guest export {name}"))
        };
        let command = index("invoke-command")?;
        let completion = index("invoke-completion")?;
        let shortcut = index("invoke-shortcut")?;
        Ok(Self {
            command: instance.get_typed_func(&mut *store, command)?,
            completion: instance.get_typed_func(&mut *store, completion)?,
            shortcut: instance.get_typed_func(&mut *store, shortcut)?,
        })
    }

    /// Transfers original caller operands rather than selecting a current native context.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: RegisteredCall<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            RegisteredCall::Command(handler, args, context, reply) => {
                let (callback, context) = arguments(accessor, &handler, CommandContext(context))?;
                let result = self
                    .command
                    .call_concurrent(
                        accessor,
                        (Resource::new_borrow(callback.rep()), args, context),
                    )
                    .await;
                accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
                let _ = reply.send(result?.0);
            }
            RegisteredCall::Shortcut(handler, context, reply) => {
                let result = session(accessor, self.shortcut, handler, Context(context)).await?;
                let _ = reply.send(result);
            }
            RegisteredCall::Completion(handler, prefix, reply) => {
                let callback = accessor.with(|mut access| {
                    access.get().ctx().table.push(Callback { id: handler.id })
                })?;
                let result = self
                    .completion
                    .call_concurrent(accessor, (Resource::new_borrow(callback.rep()), prefix))
                    .await;
                accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
                let result = result?
                    .0
                    .map(|items| items.map(|items| items.into_iter().map(Into::into).collect()));
                let _ = reply.send(result);
            }
        }
        Ok(())
    }
}
