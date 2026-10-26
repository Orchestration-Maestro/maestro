//! One owned operating environment shared by both standard-interface generations.
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

/// Copies the native environment, reserving the artifact location for this instance.
pub(crate) fn build(path: &str) -> wasmtime::Result<WasiCtx> {
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
    if let Ok(cwd) = std::env::current_dir() {
        builder.initial_cwd(cwd.to_string_lossy());
    }
    #[cfg(unix)]
    builder.preopened_dir("/", "/", wasmtime_wasi::FsPerms::ReadWrite)?;
    Ok(builder.build())
}
