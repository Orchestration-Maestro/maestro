//! SSH observations supplied to the original native coding-tool owners.
use maestro_extensions_wasm::{
    BashOperations, DiagnosticErrorInfo, EditOperations, ExtensionAPI, ExtensionResult,
    OperationFuture, ReadOperations, ToolsOptions, WriteOperations, tools::path_utils,
};
use std::rc::Rc;

use super::transport::{execute, quote};

/// One resolved destination and the local prefix its operations replace.
#[derive(Clone)]
pub(super) struct Remote {
    /// Host operations retained independently of a callback.
    pub api: ExtensionAPI,
    /// Literal SSH destination, never a local shell fragment.
    pub host: String,
    /// Resolved remote working directory.
    pub cwd: String,
    /// Local directory represented by the remote working directory.
    pub local_cwd: String,
}

impl Remote {
    /// Maps a complete host directory prefix to the remote directory’s POSIX spelling.
    ///
    /// # Errors
    /// Returns an error if host path resolution fails or returns an invalid response.
    fn path(&self, path: &str) -> ExtensionResult<String> {
        let local_cwd = path_utils::resolve(&self.api, &[&self.local_cwd])?;
        let local_path = path_utils::resolve(&self.api, &[path])?;
        let windows = !local_cwd.starts_with('/')
            && (local_cwd.starts_with("\\\\") || local_cwd.as_bytes().get(1) == Some(&b':'));
        let local_cwd = if windows {
            local_cwd.replace('\\', "/")
        } else {
            local_cwd
        };
        let local_path = if windows {
            local_path.replace('\\', "/")
        } else {
            local_path
        };
        Ok(local_path
            .strip_prefix(local_cwd.trim_end_matches('/'))
            .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
            .map_or_else(|| path.to_owned(), |suffix| format!("{}{suffix}", self.cwd)))
    }

    /// Carries child diagnostics through the tool owner's structured error interface.
    fn operation(
        &self,
        path: &str,
        command: &str,
        input: Option<String>,
    ) -> OperationFuture<'static, Vec<u8>> {
        let path = self.path(path);
        let command = command.to_owned();
        let remote = self.clone();
        Box::pin(async move {
            let path = path.map_err(DiagnosticErrorInfo::error)?;
            let command = format!("{command} {}", quote(&path));
            execute(&remote.api, &remote.host, command, input)
                .await
                .map_err(DiagnosticErrorInfo::error)
        })
    }

    /// Discards successful output for access checks and file-changing operations.
    fn unit(
        &self,
        path: &str,
        command: &str,
        input: Option<String>,
    ) -> OperationFuture<'static, ()> {
        let work = self.operation(path, command, input);
        Box::pin(async move { work.await.map(|_| ()) })
    }

    /// Retains raw bytes and the four image types recognized by the remote classifier.
    fn read(&self) -> ReadOperations {
        let read = self.clone();
        let access = self.clone();
        let image = self.clone();
        ReadOperations {
            read_file: Rc::new(move |path| read.operation(&path, "cat --", None)),
            access: Rc::new(move |path| access.unit(&path, "test -r", None)),
            detect_image_mime_type: Some(Rc::new(move |path| image.image(&path))),
        }
    }

    /// Treats unavailable or unrecognized image metadata as text.
    fn image(&self, path: &str) -> OperationFuture<'static, Option<String>> {
        let work = self.operation(path, "file --mime-type -b --", None);
        Box::pin(async move { Ok(work.await.ok().and_then(|bytes| classify(&bytes))) })
    }

    /// Sends UTF-8 content over stdin rather than embedding it in a remote command.
    fn write(&self) -> WriteOperations {
        let write = self.clone();
        let mkdir = self.clone();
        WriteOperations {
            write_file: Rc::new(move |path, content| write.unit(&path, "cat >", Some(content))),
            mkdir: Rc::new(move |path| mkdir.unit(&path, "mkdir -p --", None)),
        }
    }

    /// Reuses native child streaming, cancellation and deadlines for the SSH command.
    pub fn bash(&self) -> ExtensionResult<BashOperations> {
        let local = self.api.create_local_bash_operations(None)?;
        let remote = self.clone();
        Ok(BashOperations {
            exec: Rc::new(move |command, cwd, options| {
                let path = remote.path(&cwd);
                let cwd = remote.local_cwd.clone();
                let host = remote.host.clone();
                let exec = Rc::clone(&local.exec);
                Box::pin(async move {
                    let command = format!("cd {} && {command}", quote(&path?));
                    let command = format!("exec ssh -- {} {}", quote(&host), quote(&command));
                    exec(command, cwd, options).await
                })
            }),
        })
    }

    /// Supplies only the selected tool's effects; all algorithms stay with their owners.
    pub fn options(&self, name: &str) -> ExtensionResult<ToolsOptions> {
        let mut options = ToolsOptions::default();
        match name {
            "read" => options.read.operations = Some(self.read()),
            "write" => options.write.operations = Some(self.write()),
            "edit" => {
                let read = self.read();
                options.edit.operations = Some(EditOperations {
                    read_file: read.read_file,
                    access: read.access,
                    write_file: self.write().write_file,
                });
            }
            "bash" => options.bash.operations = Some(self.bash()?),
            _ => return Err(format!("Unknown remote tool: {name}")),
        }
        Ok(options)
    }
}

/// Selects only the image MIME types the remote reader supports.
fn classify(bytes: &[u8]) -> Option<String> {
    let mime = String::from_utf8_lossy(bytes).trim().to_owned();
    ["image/jpeg", "image/png", "image/gif", "image/webp"]
        .contains(&mime.as_str())
        .then_some(mime)
}
