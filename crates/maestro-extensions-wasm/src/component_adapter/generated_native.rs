//! Generated native resource calls, with lifecycle retained by their host owners.
use super::{Generated, host};
use crate::{ExtensionFuture, ExtensionResult};

impl crate::component_adapter::native_imports::NativeImports for Generated {
    type Watcher = host::FsWatcher;
    type Signal = host::AbortSignal;
    fn fetch<'a>(
        &'a self,
        request: host::HttpRequest,
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, Self::Response> {
        Box::pin(host::fetch(request, signal))
    }
    fn sleep<'a>(
        &'a self,
        duration: (u64, u32),
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(host::sleep(duration, signal))
    }

    type Response = host::Response;
    fn response_json<'a>(
        &'a self,
        response: &'a Self::Response,
    ) -> ExtensionFuture<'a, serde_json::Value> {
        Box::pin(async move {
            let value = response.json().await?;
            serde_json::from_str(&value).map_err(|error| error.to_string())
        })
    }
    type Child = host::ChildProcess;
    type Readable = host::Readable;
    type Writable = host::Writable;
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, Self::Child> {
        Box::pin(async move {
            let [stdin, stdout, stderr] = options.stdio.map(native_stdio);
            let options = host::SpawnOptions {
                cwd: options.cwd,
                env: options.env,
                detached: options.detached,
                stdio: (stdin, stdout, stderr),
            };
            host::spawn(command, args, options).await
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> ExtensionResult<()> {
        host::kill(pid, signal.as_deref())
    }
    fn child_kill(&self, child: &Self::Child, signal: Option<String>) -> ExtensionResult<bool> {
        child.kill(signal.as_deref())
    }
    fn child_wait<'a>(
        &'a self,
        child: &'a Self::Child,
    ) -> ExtensionFuture<'a, crate::child_process::ExitStatus> {
        Box::pin(async move {
            let status = child.wait().await?;
            Ok(crate::child_process::ExitStatus {
                code: status.code,
                signal: status.signal,
            })
        })
    }
    forward_resources! {
        watcher_close(watcher: &Self::Watcher) -> ExtensionResult<()> => close();
        response_status(response: &Self::Response) -> u16 => status();
        response_status_text(response: &Self::Response) -> String => status_text();
        response_headers(response: &Self::Response) -> Vec<(String, String)> => headers();
        response_body(response: &Self::Response) -> Option<Self::Readable> => body();
        child_pid(child: &Self::Child) -> Option<u32> => pid();
        child_stdin(child: &Self::Child) -> Option<Self::Writable> => stdin();
        child_stdout(child: &Self::Child) -> Option<Self::Readable> => stdout();
        child_stderr(child: &Self::Child) -> Option<Self::Readable> => stderr();
        readable_close(reader: &Self::Readable) -> ExtensionResult<()> => close();
    }
    forward_resources! { asynchronous {
        response_text<'a>(response: Self::Response) -> String => text();
        readable_read<'a>(reader: Self::Readable) -> Option<Vec<u8>> => read();
        writable_write<'a>(writer: Self::Writable, bytes: &'a [u8]) -> () => write(bytes.to_vec());
        writable_end<'a>(writer: Self::Writable) -> () => end();
    } }
}

/// Projects descriptor selection without changing omission or pipe presence.
fn native_stdio(value: crate::child_process::Stdio) -> host::Stdio {
    match value {
        crate::child_process::Stdio::Pipe => host::Stdio::Pipe,
        crate::child_process::Stdio::Ignore => host::Stdio::Ignore,
        crate::child_process::Stdio::Inherit => host::Stdio::Inherit,
    }
}
