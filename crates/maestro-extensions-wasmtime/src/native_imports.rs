//! Canonical native resources forward typed bytes through the existing local pump.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::{HostChildProcess, HostReadable},
    context_imports::add_resource,
    lifecycle::State,
    local_resources::{
        Message,
        native_resources::{Child, Reader, Request, Spawn, Writer},
    },
    read_imports::{endpoint, forwards, release},
};
use maestro_extensions::native_io;
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource},
};
use wasmtime_wasi::WasiView;

/// Canonical standard descriptor selection.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by Wasmtime Lift from wire values; host code only matches them"
)]
enum WireStdio {
    /// Retained byte pipe.
    #[component(name = "pipe")]
    Pipe,
    /// Null descriptor.
    #[component(name = "ignore")]
    Ignore,
    /// Inherited native descriptor.
    #[component(name = "inherit")]
    Inherit,
}
impl From<WireStdio> for native_io::Stdio {
    fn from(value: WireStdio) -> Self {
        match value {
            WireStdio::Pipe => Self::Pipe,
            WireStdio::Ignore => Self::Ignore,
            WireStdio::Inherit => Self::Inherit,
        }
    }
}
/// Canonical launch selections preserve absent and present-empty values.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Options {
    /// Native directory, absent unless supplied.
    cwd: Option<String>,
    /// Complete optional environment.
    env: Option<Vec<(String, String)>>,
    /// Native session selection.
    detached: bool,
    /// Independent input/output/error selections.
    stdio: (WireStdio, WireStdio, WireStdio),
}
impl From<Options> for native_io::SpawnOptions {
    fn from(options: Options) -> Self {
        Self {
            cwd: options.cwd,
            env: options.env,
            detached: options.detached,
            stdio: [
                options.stdio.0.into(),
                options.stdio.1.into(),
                options.stdio.2.into(),
            ],
        }
    }
}
/// Canonical numeric exit or terminating signal.
#[derive(ComponentType, Lift, Lower)]
#[component(record)]
struct Exit {
    /// Absent on signal termination.
    code: Option<i32>,
    /// Absent on ordinary numeric exit.
    signal: Option<String>,
}

/// Installs each typed native resource against its original local owner.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Writer>(host, "writable")?;
    crate::http_imports::add_to_linker(host)?;
    crate::watch_imports::add_to_linker(host)?;
    crate::file_imports::add_to_linker(host)?;
    launch(host)?;
    controls(host)?;
    reads(host)?;
    writes(host)
}
/// Starts native launches and explicit PID signalling without JSON identities.
fn launch<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "spawn",
        |accessor, (command, args, options): (String, Vec<String>, Options)| {
            Box::pin(async move {
                let api = accessor.with(|mut access| access.get().api().clone());
                let result = api
                    .0
                    .request_async(|_, reply, admission| {
                        Message::Native(Request::Spawn(
                            Spawn {
                                command,
                                args,
                                options: options.into(),
                            },
                            reply,
                            admission,
                        ))
                    })
                    .await;
                let result = match result {
                    Ok(child) => Ok(accessor.with(|mut a| a.get().ctx().table.push(child))?),
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )?;
    host.func_wrap(
        "kill",
        |store: StoreContextMut<'_, T>, (pid, signal): (i32, Option<String>)| {
            let result = store
                .data()
                .api()
                .0
                .request(|_, reply| Message::Native(Request::Kill(pid, signal, reply)));
            Ok((result,))
        },
    )
}
/// Waits on the original child; only the native owner waits and reaps.
fn controls<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]child-process.wait",
        |accessor, (child,): (Resource<Child>,)| {
            Box::pin(async move {
                let child = accessor.with(|mut a| a.get().ctx().table.get(&child).cloned())?;
                let result = child.wait().await;
                Ok((result.map(|status| Exit {
                    code: status.code,
                    signal: status.signal,
                }),))
            })
        },
    )
}
/// Reads only this consumer's bytes.
fn reads<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]readable.read",
        |accessor, (reader,): (Resource<Reader>,)| {
            Box::pin(async move {
                let reader = accessor.with(|mut a| a.get().ctx().table.get(&reader).cloned())?;
                Ok((reader.read().await,))
            })
        },
    )
}

/// Admits ordered native writes without blocking stdout, stderr or child controls.
fn writes<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]writable.write",
        |accessor, (writer, bytes): (Resource<Writer>, Vec<u8>)| {
            Box::pin(async move {
                let writer = accessor.with(|mut a| a.get().ctx().table.get(&writer).cloned())?;
                Ok((writer.write(bytes).await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]writable.end",
        |accessor, (writer,): (Resource<Writer>,)| {
            Box::pin(async move {
                let writer = accessor.with(|mut a| a.get().ctx().table.get(&writer).cloned())?;
                Ok((writer.end().await,))
            })
        },
    )
}

impl HostChildProcess for State {
    fn pid(&mut self, this: Resource<Child>) -> wasmtime::Result<Option<u32>> {
        endpoint(self, &this)?.pid().map_err(wasmtime::Error::msg)
    }

    fn stdin(&mut self, this: Resource<Child>) -> wasmtime::Result<Option<Resource<Writer>>> {
        let pipe = endpoint(self, &this)?
            .stdin()
            .map_err(wasmtime::Error::msg)?;
        Ok(pipe.map(|pipe| self.ctx().table.push(pipe)).transpose()?)
    }

    fn stdout(&mut self, this: Resource<Child>) -> wasmtime::Result<Option<Resource<Reader>>> {
        output(self, &this, false)
    }

    fn stderr(&mut self, this: Resource<Child>) -> wasmtime::Result<Option<Resource<Reader>>> {
        output(self, &this, true)
    }

    fn kill(
        &mut self,
        this: Resource<Child>,
        signal: Option<String>,
    ) -> wasmtime::Result<Result<bool, String>> {
        Ok(endpoint(self, &this)?.kill(signal))
    }

    fn drop(&mut self, this: Resource<Child>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Acquires one output pipe of a child and lends it to the guest.
fn output(
    state: &mut State,
    this: &Resource<Child>,
    stderr: bool,
) -> wasmtime::Result<Option<Resource<Reader>>> {
    let pipe = endpoint(state, this)?
        .output(stderr)
        .map_err(wasmtime::Error::msg)?;
    Ok(pipe.map(|pipe| state.ctx().table.push(pipe)).transpose()?)
}

impl HostReadable for State {
    fn close(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.close())
    }

    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the synchronous methods of a child and its byte reader to their traits.
pub(crate) fn add_pipes(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Child>(host, "child-process")?;
    add_resource::<State, Reader>(host, "readable")?;
    forwards! { host, "child-process", HostChildProcess, Child,
        "pid" => pid();
        "stdin" => stdin();
        "stdout" => stdout();
        "stderr" => stderr();
        "kill" => kill(signal: Option<String>);
    }
    forwards! { host, "readable", HostReadable, Reader,
        "close" => close();
    }
    Ok(())
}
