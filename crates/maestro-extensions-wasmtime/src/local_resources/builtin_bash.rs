//! Native shell construction retains the original backend and synchronous callback frame.
use super::{
    Capability, Message, Registry,
    builtin::{Factory, Operation},
};
use crate::guest_calls::{Handle, bash::ReturnedBash};
use maestro_extensions::{BashSpawnContext, DiagnosticErrorInfo, Local};
use serde::{Deserialize, Serialize};
use std::{rc::Rc, sync::RwLock};
use wasmtime::component::{ComponentType, Lift};

/// Independent non-callback controls supplied by the guest factory.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Controls {
    /// Nonempty literal command prefix.
    #[component(name = "command-prefix")]
    pub(crate) command_prefix: Option<String>,
    /// Explicit local shell executable.
    #[component(name = "shell-path")]
    pub(crate) shell_path: Option<String>,
}

/// Retained callbacks never enter the encoded operand document.
pub(crate) struct Options {
    /// Independent command controls.
    pub(crate) controls: Controls,
    /// Original returned-shell callback owner.
    pub(crate) operations: Option<Handle>,
    /// Synchronous interception using the original preparation callback frame.
    pub(crate) hook: Option<Handle>,
}

impl crate::api_reads::Endpoint {
    /// Retains a native shell without creating a process or invoking the hook.
    pub(crate) fn create_bash(
        &self,
        cwd: String,
        options: Options,
    ) -> Result<crate::builtin_reads::Endpoint, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::Create(Factory::Bash(id, cwd, options), reply))
        })
    }
}

impl Registry {
    /// Builds the original typed definition against the application's binary directory.
    pub(super) fn bash(
        &self,
        api: u64,
        cwd: String,
        options: Options,
    ) -> Result<Rc<dyn crate::builtin_tools::Definition>, String> {
        let services = self.tool_construction(api)?;
        let options = self.bash_options(api, options)?;
        Ok(Rc::new(RwLock::new(
            maestro_extensions::local::create_bash_tool_definition::<crate::builtin_tools::Context>(
                cwd,
                services.bin_dir,
                options,
            ),
        )))
    }

    /// Retains shell effects under the same emitter for individual and named factories.
    pub(super) fn bash_options(
        &self,
        api: u64,
        options: Options,
    ) -> Result<maestro_extensions::local::BashToolOptions, String> {
        let capability = self.capability(api);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            return Err("extension API resource ended".into());
        };
        Ok(maestro_extensions::local::BashToolOptions {
            command_prefix: options.controls.command_prefix,
            shell_path: options.controls.shell_path,
            operations: options.operations.map(|owner| {
                Rc::new(ReturnedBash::new(owner, self.clone()))
                    as Rc<dyn maestro_extensions::BashOperations<Local>>
            }),
            spawn_hook: options.hook.map(|owner| {
                let prepare = owner.prepare(self.clone(), api.events.clone());
                Rc::new(move |context| intercept(context, &prepare))
                    as maestro_extensions::local::BashSpawnHook
            }),
        })
    }
}

/// UTF-8 shell operands use the existing preparation document, not a resource-ID encoding.
#[derive(Serialize, Deserialize)]
struct Spawn {
    /// Literal command after prefixing.
    command: String,
    /// Literal execution directory.
    cwd: String,
    /// Ordered environment, including unset values.
    env: serde_json::Map<String, serde_json::Value>,
}

/// Runs the authored replacement synchronously under the existing borrowed emitter.
fn intercept(
    context: BashSpawnContext,
    prepare: &maestro_extensions::PrepareArguments<Local>,
) -> Result<BashSpawnContext, DiagnosticErrorInfo> {
    let env = context
        .env
        .iter()
        .map(crate::guest_calls::bash::environment_entry)
        .map(|entry| {
            entry.map(|(key, value)| {
                (
                    key,
                    value.map_or(serde_json::Value::Null, serde_json::Value::String),
                )
            })
        })
        .collect::<Result<_, _>>()
        .map_err(DiagnosticErrorInfo::error)?;
    let document = Spawn {
        command: context.command,
        cwd: context.cwd,
        env,
    };
    let mut document = serde_json::to_value(document)
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
    let result = prepare(&mut document)?;
    let result: Spawn = serde_json::from_value(result)
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
    let env = result
        .env
        .into_iter()
        .map(|(key, value)| {
            let value: Option<String> = serde_json::from_value(value)
                .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
            Ok((key.into(), value.map(Into::into)))
        })
        .collect::<Result<_, DiagnosticErrorInfo>>()?;
    Ok(BashSpawnContext {
        command: result.command,
        cwd: result.cwd,
        env,
    })
}
