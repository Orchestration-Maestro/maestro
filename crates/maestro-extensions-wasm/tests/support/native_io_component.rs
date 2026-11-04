//! Exercises native operations through the ordinary author facade.
// maestro-component: MAESTRO_NATIVE_IO_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/children.rs"]
mod children;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/files.rs"]
mod files;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/http.rs"]
mod http;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/waits.rs"]
mod waits;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/watch.rs"]
mod watch;

/// Registers independently callable native operations.
#[cfg(target_arch = "wasm32")]
fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        children::register(&api, Rc::clone(&io))?;
        http::register(&api, Rc::clone(&io))?;
        watch::register(&api, Rc::clone(&io))?;
        files::register(&api, Rc::clone(&io))?;
        let direct = Rc::clone(&io);
        api.register_command(
            "native-wait",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    let io = Rc::clone(&direct);
                    Box::pin(async move {
                        let millis = args.parse::<u64>().map_err(|e| e.to_string())?;
                        waits::begin(io, &context, millis)?.finish().await
                    })
                }),
            },
        )?;
        let pending = Rc::new(std::cell::RefCell::new(None));
        api.register_command(
            "native-retained-wait",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    let (io, pending) = (Rc::clone(&io), Rc::clone(&pending));
                    Box::pin(async move { waits::retained(io, &pending, &context, &args).await })
                }),
            },
        )?;
        if let Ok(paths) = std::env::var("MAESTRO_NATIVE_IO_BASICS") {
            full_user_basics(&paths).map_err(|error| error.to_string())?;
        }
        Ok(())
    })
}

/// Uses inherited standard facilities before the frontend starts reading stdin.
#[cfg(target_arch = "wasm32")]
fn full_user_basics(paths: &str) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let paths: [String; 2] = serde_json::from_str(paths).map_err(std::io::Error::other)?;
    let mut stdout = std::io::stdout().lock();
    let sentinel = std::env::var("NATIVE_IO_SENTINEL").map_err(std::io::Error::other)?;
    stdout.write_all(sentinel.as_bytes())?;
    for path in paths {
        stdout.write_all(&std::fs::read(&path)?)?;
        std::fs::write(path, b"changed\0\xff\xe9\x9b\xaa")?;
    }
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    stdout.write_all(b"OUT[")?;
    stdout.write_all(&input)?;
    stdout.write_all(b"]")?;
    stdout.flush()?;
    input.reverse();
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(b"ERR[")?;
    stderr.write_all(&input)?;
    stderr.write_all(b"]")
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
