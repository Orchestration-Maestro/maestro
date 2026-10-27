//! One owned operating environment shared by both standard-interface generations.
use std::{
    io::{self, Write},
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
/// Throwaway qualification layouts selected by a marker file in the native cwd.
#[cfg(windows)]
fn probe(builder: &mut WasiCtxBuilder) -> wasmtime::Result<()> {
    use maestro_extensions::helpers::guest_path;
    let Some(cwd) = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.to_str().map(str::to_owned))
    else {
        return Ok(());
    };
    let root = std::path::Path::new(&cwd).ancestors().last().unwrap().to_str().unwrap().to_owned();
    eprintln!("PROBE host cwd-root={root:?} guest-cwd={:?}", guest_path(&cwd));
    if std::path::Path::new("probe-root-layout").exists() {
        builder.preopened_dir(&root, "/", wasmtime_wasi::FsPerms::ReadWrite)?;
        return Ok(());
    }
    for root in ('A'..='Z').map(|letter| format!("{letter}:\\")).chain([root]) {
        let name = guest_path(&root).trim_end_matches('/').to_owned();
        let opened = builder.preopened_dir(&root, &name, wasmtime_wasi::FsPerms::ReadWrite).map(|_| ());
        eprintln!("PROBE host root={root:?} name={name:?} opened={opened:?}");
    }
    builder.initial_cwd(guest_path(&cwd));
    Ok(())
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
    if cfg!(unix) {
        if let Ok(cwd) = std::env::current_dir()
            && let Some(cwd) = cwd.to_str()
        {
            builder.initial_cwd(cwd);
        }
        builder.preopened_dir("/", "/", wasmtime_wasi::FsPerms::ReadWrite)?;
    }
    #[cfg(windows)]
    probe(&mut builder)?;
    if let Some(output) = output {
        builder.stdout(output);
    }
    Ok(builder.build())
}
