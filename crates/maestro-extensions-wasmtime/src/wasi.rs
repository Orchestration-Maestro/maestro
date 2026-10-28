//! One owned operating environment shared by both standard-interface generations.
use maestro_extensions::helpers::guest_path;
use std::{
    collections::BTreeSet,
    io::{self, Write},
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::io::AsyncWrite;
use wasmtime_wasi::{
    WasiCtx, WasiCtxBuilder,
    cli::{IsTerminal, StdoutStream},
};

/// One caller-owned writer shared by all output handles of an instance.
#[derive(Clone)]
pub(crate) struct Output {
    /// The caller's retained writer.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Whether the writer represents a terminal.
    terminal: bool,
}
impl Output {
    /// Retains the supplied writer independently of the factory call.
    pub(crate) fn new(writer: Box<dyn Write + Send>, terminal: bool) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            terminal,
        }
    }
}
impl IsTerminal for Output {
    fn is_terminal(&self) -> bool {
        self.terminal
    }
}
impl StdoutStream for Output {
    fn async_stream(&self) -> Box<dyn AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}
impl AsyncWrite for Output {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(
            self.writer
                .lock()
                .map_err(|e| io::Error::other(e.to_string()))
                .and_then(|mut writer| writer.write_all(bytes).map(|()| bytes.len())),
        )
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(
            self.writer
                .lock()
                .map_err(|e| io::Error::other(e.to_string()))
                .and_then(|mut writer| writer.flush()),
        )
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
/// Opens every accessible drive and the cwd's own root, such as a share, under its guest name.
///
/// Windows has no single root. A root that cannot be opened leaves the others accessible.
fn windows_roots(builder: &mut WasiCtxBuilder, cwd: Option<&str>) {
    let cwd_root = cwd.and_then(|cwd| Path::new(cwd).ancestors().last()?.to_str());
    let roots: BTreeSet<_> = ('A'..='Z')
        .map(|letter| format!(r"{letter}:\"))
        .chain(cwd_root.map(str::to_owned))
        .collect();
    for root in roots {
        let name = guest_path(&root);
        let _unavailable = builder.preopened_dir(
            &root,
            name.trim_end_matches('/'),
            wasmtime_wasi::FsPerms::ReadWrite,
        );
    }
}
/// Copies the native environment, reserving the artifact location for this instance.
pub(crate) fn build(path: &str, output: Option<Output>) -> wasmtime::Result<WasiCtx> {
    let env: Vec<_> = std::env::vars_os()
        .filter(|(key, _)| {
            if cfg!(windows) {
                !key.to_string_lossy()
                    .eq_ignore_ascii_case("MAESTRO_EXTENSION_PATH")
            } else {
                key != "MAESTRO_EXTENSION_PATH"
            }
        })
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let mut builder = WasiCtxBuilder::new();
    builder
        .envs(&env)
        .env("MAESTRO_EXTENSION_PATH", path)
        .inherit_stdin()
        .inherit_stdout()
        .inherit_stderr()
        .inherit_network()
        .allow_ip_name_lookup(true)
        .allow_tcp(true)
        .allow_udp(true);
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.to_str().map(str::to_owned));
    if cfg!(unix) {
        builder.preopened_dir("/", "/", wasmtime_wasi::FsPerms::ReadWrite)?;
    } else if cfg!(windows) {
        windows_roots(&mut builder, cwd.as_deref());
    }
    if let Some(cwd) = cwd {
        builder.initial_cwd(guest_path(&cwd));
    }
    if let Some(output) = output {
        builder.stdout(output);
    }
    Ok(builder.build())
}
