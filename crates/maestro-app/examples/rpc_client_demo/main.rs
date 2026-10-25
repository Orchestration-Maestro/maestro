//! An interactive terminal client of a Maestro child in RPC mode.
//!
//! Pass the path of a built Maestro executable; without one the client runs
//! `dist/maestro`. Type a prompt per line, `exit` to quit; an interrupt aborts the running
//! turn, or quits when none runs.
#[cfg(not(target_arch = "wasm32"))]
mod interactive;

#[cfg(not(target_arch = "wasm32"))]
use maestro_app::{RpcClient, RpcClientOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, rc::Rc};
#[cfg(not(target_arch = "wasm32"))]
use tokio::{io::AsyncBufReadExt, sync::mpsc};

/// Wires standard input, interrupts and standard output to the flow.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let client = Rc::new(RpcClient::new(RpcClientOptions {
        cli_path: std::env::args().nth(1),
        provider: Some("anthropic".into()),
        model: Some("claude-sonnet-4-20250514".into()),
        args: vec!["--no-session".into()],
        ..RpcClientOptions::default()
    }));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let output: interactive::Output = Rc::new(RefCell::new(std::io::stdout()));
    let result = tokio::task::LocalSet::new().block_on(&runtime, async {
        let (lines, input) = mpsc::unbounded_channel();
        let (interrupt, interrupts) = mpsc::unbounded_channel();
        tokio::task::spawn_local(async move {
            let mut reader = tokio::io::BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = lines.send(line);
            }
        });
        tokio::task::spawn_local(async move {
            while tokio::signal::ctrl_c().await.is_ok() {
                let _ = interrupt.send(());
            }
        });
        interactive::run(client, input, interrupts, output).await
    });
    interactive::report(result, &mut std::io::stderr());
    // The standard input reader blocks a thread that must not delay the exit.
    runtime.shutdown_background();
    Ok(())
}

/// The client controls a native child, which browser builds do not have.
#[cfg(target_arch = "wasm32")]
fn main() {}
