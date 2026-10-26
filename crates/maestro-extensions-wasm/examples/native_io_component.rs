//! Exercises native operations through the ordinary author facade.
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
#[path = "native_io_component/http.rs"]
mod http;
#[cfg(target_arch = "wasm32")]
#[path = "native_io_component/waits.rs"]
mod waits;

/// Registers independently callable native operations.
#[cfg(target_arch = "wasm32")]
fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        children::register(&api, Rc::clone(&io))?;
        http::register(&api, Rc::clone(&io))?;
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
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
