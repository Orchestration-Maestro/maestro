//! Read-only Git availability and recognition of remote hash records.
use super::{CommandCaptureOptions, DefaultPackageManager, PackageOperations};
use std::{io, time::Duration};

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Suppresses local and remote probe failures without changing the checkout.
    pub(super) async fn git_update_available(&self, path: &str) -> bool {
        if self.offline() {
            return false;
        }
        let result = async {
            let local = self.git_capture(path, &["rev-parse", "HEAD"], &[]).await?;
            let remote = self.remote_git_head(path).await?;
            Ok::<_, io::Error>(crate::trim(&local) != crate::trim(&remote))
        }
        .await;
        result.unwrap_or(false)
    }
    /// Uses a supported origin upstream before falling back to the remote HEAD.
    pub(super) async fn remote_git_head(&self, path: &str) -> io::Result<String> {
        let upstream = self
            .git_capture(path, &["rev-parse", "--abbrev-ref", "@{upstream}"], &[])
            .await
            .ok();
        let branch = upstream
            .as_deref()
            .map(crate::trim)
            .and_then(|text| text.strip_prefix("origin/"))
            .filter(|text| !text.is_empty());
        if let Some(branch) = branch {
            let reference = format!("refs/heads/{branch}");
            let output = self
                .git_capture(
                    path,
                    &["ls-remote", "origin", &reference],
                    &[("GIT_TERMINAL_PROMPT", "0")],
                )
                .await?;
            if let Some(hash) = remote_hash(&output, false) {
                return Ok(hash.to_owned());
            }
        }
        let output = self
            .git_capture(
                path,
                &["ls-remote", "origin", "HEAD"],
                &[("GIT_TERMINAL_PROMPT", "0")],
            )
            .await?;
        remote_hash(&output, true)
            .map(str::to_owned)
            .ok_or_else(|| io::Error::other("Failed to determine remote HEAD"))
    }
    /// Supplies the common Git network capture operands at the consuming call.
    async fn git_capture(
        &self,
        path: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> io::Result<String> {
        let args: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
        self.operations
            .run_command_capture(
                "git",
                &args,
                CommandCaptureOptions {
                    cwd: Some(path),
                    timeout: Some(Duration::from_millis(10000)),
                    env,
                },
            )
            .await
    }
}
/// Recognizes line-start lowercase hashes in the original output, retaining multiline whitespace.
fn remote_hash(text: &str, require_head: bool) -> Option<&str> {
    text.char_indices()
        .filter(|(index, _)| {
            *index == 0 || text[..*index].ends_with(['\r', '\n', '\u{2028}', '\u{2029}'])
        })
        .find_map(|(index, _)| {
            let rest = &text[index..];
            let hash = rest.get(..40)?;
            if !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return None;
            }
            let after = &rest[40..];
            let whitespace = after.len() - after.trim_start_matches(crate::whitespace).len();
            if whitespace == 0 {
                return None;
            }
            if require_head {
                let suffix = after[whitespace..].strip_prefix("HEAD")?;
                if !suffix.is_empty() && !suffix.starts_with(['\r', '\n', '\u{2028}', '\u{2029}']) {
                    return None;
                }
            }
            Some(hash)
        })
}
