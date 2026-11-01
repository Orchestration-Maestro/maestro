//! The share and copy commands: a secret gist of the exported session, and the last
//! answer on the clipboard.

use super::InteractiveMode;
use crate::{BorderedLoader, BorderedLoaderOptions};
use maestro_app::configuration::config::{SERVICE_ORIGIN, get_share_viewer_url};
use maestro_app::{
    CapturedChild, OutputMode, SpawnSyncOptions, Termination, WaitOptions, is_whitespace_scalar,
    spawn_sync, wait_for_child_process,
};
use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::process::Command;

/// The result of one command the mode runs.
type CommandResult = Result<(), Box<dyn Error>>;

/// The number of the next share operation of this process.
static NEXT_SHARE: AtomicU64 = AtomicU64::new(0);

/// The exclusive directory and HTML file of one share operation.
pub(super) struct ShareArtifact {
    /// The directory this operation created and alone removes.
    pub(super) directory: PathBuf,
    /// The exported document inside that directory.
    pub(super) file: PathBuf,
}

impl ShareArtifact {
    /// Creates this process's next artifact directory, which must not exist yet.
    ///
    /// # Errors
    /// Returns the native directory-creation error, including an existing directory.
    pub(super) fn new() -> io::Result<Self> {
        let directory = named(NEXT_SHARE.fetch_add(1, Ordering::Relaxed));
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt as _;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = fs::DirBuilder::new();
        builder.create(&directory)?;
        Ok(Self {
            file: directory.join("session.html"),
            directory,
        })
    }

    /// The directory this process's next share operation would create.
    #[cfg(all(test, unix))]
    pub(super) fn next_directory() -> PathBuf {
        named(NEXT_SHARE.load(Ordering::Relaxed))
    }

    /// Removes the document and then its directory, ignoring every failure so a cleanup
    /// never replaces the operation's result.
    pub(super) fn remove(&self) {
        drop(fs::remove_file(&self.file));
        drop(fs::remove_dir(&self.directory));
    }
}

/// The artifact directory of operation number `sequence` of this process.
fn named(sequence: u64) -> PathBuf {
    std::env::temp_dir().join(format!("maestro-share-{}-{sequence}", std::process::id()))
}

impl Drop for ShareArtifact {
    fn drop(&mut self) {
        self.remove();
    }
}

impl InteractiveMode {
    /// Copies the last answer, or reports that there is nothing to copy yet.
    ///
    /// # Errors
    /// Returns the failure of showing the notice.
    pub(super) async fn handle_copy_command(&self) -> CommandResult {
        let Some(text) = self.runtime_host.session().get_last_assistant_text() else {
            return Ok(self.show_error("No agent messages to copy yet.")?);
        };
        match maestro_app::copy_to_clipboard(&text).await {
            Ok(()) => self
                .show_status("Copied last agent message to clipboard")
                .map_err(Into::into),
            Err(error) => Ok(self.show_error(&error.to_string())?),
        }
    }

    /// Shares the current session as a secret gist, or reports the step that failed.
    ///
    /// # Errors
    /// Returns the failure of showing the loader or of a notice.
    pub(super) async fn handle_share_command(self: &Rc<Self>) -> CommandResult {
        match Self::check_github_cli() {
            Ok(()) => (),
            Err(notice) => return Ok(self.show_error(notice)?),
        }
        let artifact = match self.export_share() {
            Ok(artifact) => Rc::new(artifact),
            Err(error) => {
                return Ok(self.show_error(&format!("Failed to export session: {error}"))?);
            }
        };
        let loader = self.show_share_loader(Rc::clone(&artifact))?;
        let shared = Self::create_gist(&artifact, &loader).await;
        loader.set_on_abort(None);
        if loader.signal().is_aborted() {
            return Ok(());
        }
        self.restore_share_editor(&loader);
        artifact.remove();
        match shared {
            Ok(output) => self.show_share_result(&output, SERVICE_ORIGIN),
            Err(error) => Ok(self.show_error(&format!("Failed to create gist: {error}"))?),
        }
    }

    /// Runs `gh auth status`, keeping a missing program apart from a refused login.
    ///
    /// Startup failures, including a program that is absent or not executable, select the
    /// installation notice; a reached nonzero or signalled status selects the login notice.
    fn check_github_cli() -> Result<(), &'static str> {
        let options = SpawnSyncOptions::Capture {
            stdout: OutputMode::Discard,
            input: None,
            timeout: None,
            stderr: OutputMode::Discard,
            max_buffer_bytes: None,
        };
        match spawn_sync(Command::new("gh").args(["auth", "status"]), options) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(_) => Err("GitHub CLI is not logged in. Run 'gh auth login' first."),
            Err(_) => {
                Err("GitHub CLI (gh) is not installed. Install it from https://cli.github.com/")
            }
        }
    }

    /// Exports the current session into this operation's own artifact directory.
    ///
    /// # Errors
    /// Returns the artifact allocation or the export owner's failure.
    fn export_share(&self) -> Result<ShareArtifact, Box<dyn Error>> {
        let artifact = ShareArtifact::new()?;
        let path = artifact.file.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Temporary path is not valid UTF-8",
            )
        })?;
        self.export_to_html(Some(path))?;
        Ok(artifact)
    }

    /// Replaces the editor with a cancellable `Creating gist...` loader and binds
    /// cancellation to restoring the editor and reporting it.
    ///
    /// # Errors
    /// Returns the failure of the published theme or of the loader's hint.
    pub(super) fn show_share_loader(
        self: &Rc<Self>,
        artifact: Rc<ShareArtifact>,
    ) -> Result<Rc<BorderedLoader>, Box<dyn Error>> {
        let live = self.themes.theme();
        let loader = Rc::new(BorderedLoader::new(
            &self.ui,
            live.get()?,
            "Creating gist...".to_owned(),
            BorderedLoaderOptions::default(),
            &live,
        )?);
        self.show_panel(loader.clone(), loader.clone());
        let mode = Rc::downgrade(self);
        let retained = Rc::downgrade(&loader);
        loader.set_on_abort(Some(Rc::new(move || {
            if let (Some(mode), Some(loader)) = (mode.upgrade(), retained.upgrade()) {
                mode.finish_cancelled_share(&loader, &artifact);
            }
        })));
        Ok(loader)
    }

    /// Restores the editor and removes the artifact after a cancellation, then reports the
    /// cancellation through the transcript or, failing that, the runtime error route.
    fn finish_cancelled_share(&self, loader: &BorderedLoader, artifact: &ShareArtifact) {
        self.restore_share_editor(loader);
        artifact.remove();
        if let Err(error) = self.show_status("Share cancelled") {
            self.report(Err(error.into()));
        }
    }

    /// Stops the loader and puts the same editor back in focus.
    fn restore_share_editor(&self, loader: &BorderedLoader) {
        loader.dispose();
        self.restore_editor();
    }

    /// Runs `gh gist create --public=false` for the artifact, collecting both streams as
    /// raw bytes until the exit and both end-of-file events, or terminating the child when
    /// the loader is cancelled.
    ///
    /// # Errors
    /// Returns the native spawn, pipe or status failure.
    async fn create_gist(
        artifact: &ShareArtifact,
        loader: &BorderedLoader,
    ) -> io::Result<std::process::Output> {
        let mut command = Command::new("gh");
        command
            .args(["gist", "create", "--public=false"])
            .arg(&artifact.file);
        let mut child = CapturedChild::spawn(&mut command)?;
        let signal = loader.signal().clone();
        let cancellation = async move {
            signal.cancelled().await;
            Termination::Terminate
        };
        wait_for_child_process(&mut child, WaitOptions::default(), cancellation, None).await
    }

    /// Reports the created gist, decoding each captured stream once and reading the last
    /// slash segment of the printed URL as its identifier.
    pub(super) fn show_share_result(
        &self,
        output: &std::process::Output,
        service_origin: &str,
    ) -> CommandResult {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            let error = stderr.trim_matches(is_whitespace_scalar);
            let error = if error.is_empty() {
                "Unknown error"
            } else {
                error
            };
            return Ok(self.show_error(&format!("Failed to create gist: {error}"))?);
        }
        let gist = stdout.trim_matches(is_whitespace_scalar);
        let id = gist.rsplit('/').next().filter(|id| !id.is_empty());
        let Some(id) = id else {
            return Ok(self.show_error("Failed to parse gist ID from gh output")?);
        };
        let message = get_share_viewer_url(id, service_origin).map_or_else(
            || format!("Gist: {gist}"),
            |viewer| format!("Share URL: {viewer}\nGist: {gist}"),
        );
        self.show_status(&message).map_err(Into::into)
    }
}
