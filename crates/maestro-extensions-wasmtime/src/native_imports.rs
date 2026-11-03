//! Canonical native resources forward typed bytes through the existing local pump.
use crate::{
    api_imports::ApiView,
    context_imports::add_resource,
    local_resources::{
        Message,
        native_resources::{Child, Reader, Request, Spawn, Writer},
    },
};
use maestro_extensions::native_io;
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource},
};

/// Canonical standard descriptor selection.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(
    dead_code,
    reason = "constructed by Wasmtime Lift from wire values; host code only matches them"
)]
enum Stdio {
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
impl From<Stdio> for native_io::Stdio {
    fn from(value: Stdio) -> Self {
        match value {
            Stdio::Pipe => Self::Pipe,
            Stdio::Ignore => Self::Ignore,
            Stdio::Inherit => Self::Inherit,
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
    stdio: (Stdio, Stdio, Stdio),
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
    add_resource::<T, Child>(host, "child-process")?;
    add_resource::<T, Reader>(host, "readable")?;
    add_resource::<T, Writer>(host, "writable")?;
    crate::http_imports::add_to_linker(host)?;
    crate::watch_imports::add_to_linker(host)?;
    crate::file_imports::add_to_linker(host)?;
    launch(host)?;
    controls(host)?;
    pipes(host)?;
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
                    .request_async(|id, reply, admission| {
                        Message::Native(
                            id,
                            Request::Spawn(
                                Spawn {
                                    command,
                                    args,
                                    options: options.into(),
                                },
                                reply,
                                admission,
                            ),
                        )
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
                .request(|id, reply| Message::Native(id, Request::Kill(pid, signal, reply)));
            Ok((result,))
        },
    )
}
/// Reads and signals the original child; only the native owner waits and reaps.
fn controls<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]child-process.pid",
        |mut store: StoreContextMut<'_, T>, (child,): (Resource<Child>,)| {
            let child = store.data_mut().ctx().table.get(&child)?.clone();
            let result = child
                .0
                .request(|id, reply| Message::Native(id, Request::Pid(reply)))
                .map_err(wasmtime::Error::msg)?;
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "[method]child-process.kill",
        |mut store: StoreContextMut<'_, T>, (child, signal): (Resource<Child>, Option<String>)| {
            let child = store.data_mut().ctx().table.get(&child)?.clone();
            Ok((child
                .0
                .request(|id, reply| Message::Native(id, Request::ChildKill(signal, reply))),))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]child-process.wait",
        |accessor, (child,): (Resource<Child>,)| {
            Box::pin(async move {
                let child = accessor.with(|mut a| a.get().ctx().table.get(&child).cloned())?;
                let result = child
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Native(id, Request::Wait(reply, admission))
                    })
                    .await;
                Ok((result.map(|status| Exit {
                    code: status.code,
                    signal: status.signal,
                }),))
            })
        },
    )
}
/// Gives each descriptor alias independent resource ownership, not independent bytes.
fn pipes<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]child-process.stdin",
        |mut store: StoreContextMut<'_, T>, (child,): (Resource<Child>,)| {
            let child = store.data_mut().ctx().table.get(&child)?.clone();
            let pipe = child
                .0
                .request(|id, reply| Message::Native(id, Request::Stdin(reply)))
                .map_err(wasmtime::Error::msg)?;
            let pipe = pipe
                .map(|pipe| store.data_mut().ctx().table.push(pipe))
                .transpose()?;
            Ok((pipe,))
        },
    )?;
    for (name, stderr) in [
        ("[method]child-process.stdout", false),
        ("[method]child-process.stderr", true),
    ] {
        host.func_wrap(
            name,
            move |mut store: StoreContextMut<'_, T>, (child,): (Resource<Child>,)| {
                let child = store.data_mut().ctx().table.get(&child)?.clone();
                let pipe = child
                    .0
                    .request(|id, reply| Message::Native(id, Request::Output(stderr, reply)))
                    .map_err(wasmtime::Error::msg)?;
                let pipe = pipe
                    .map(|pipe| store.data_mut().ctx().table.push(pipe))
                    .transpose()?;
                Ok((pipe,))
            },
        )?;
    }
    Ok(())
}
/// Reads only this consumer's bytes; close never aborts a caller signal.
fn reads<T: ApiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]readable.read",
        |accessor, (reader,): (Resource<Reader>,)| {
            Box::pin(async move {
                let reader = accessor.with(|mut a| a.get().ctx().table.get(&reader).cloned())?;
                Ok((reader
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Native(id, Request::Read(reply, admission))
                    })
                    .await,))
            })
        },
    )?;
    host.func_wrap(
        "[method]readable.close",
        |mut store: StoreContextMut<'_, T>, (reader,): (Resource<Reader>,)| {
            let reader = store.data_mut().ctx().table.get(&reader)?.clone();
            Ok((reader
                .0
                .request(|id, reply| Message::Native(id, Request::Close(reply))),))
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
                Ok((writer
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Native(id, Request::Write(bytes, reply, admission))
                    })
                    .await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]writable.end",
        |accessor, (writer,): (Resource<Writer>,)| {
            Box::pin(async move {
                let writer = accessor.with(|mut a| a.get().ctx().table.get(&writer).cloned())?;
                Ok((writer
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Native(id, Request::End(reply, admission))
                    })
                    .await,))
            })
        },
    )
}
