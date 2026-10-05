use crate::{SettingsError, SettingsOrigin, SettingsScope};
use std::path::{Path, PathBuf};

/// Explicit locations; resolving paths never reads ambient inputs or creates files.
#[derive(Clone, Debug)]
pub struct SettingsLocations {
    working: PathBuf,
    user: PathBuf,
    home: PathBuf,
}
impl SettingsLocations {
    /// Requires absolute supplied process cwd and home; invalid roots return
    /// value-free location errors. Missing working directory uses process cwd;
    /// relative working directory uses process cwd, and relative user root uses
    /// the effective working directory. Exact `~` and `~/` expand at supplied home.
    /// No ambient lookup, canonicalization, existence check or creation occurs.
    pub fn new(
        process_cwd: PathBuf,
        working_directory: Option<PathBuf>,
        user_root: PathBuf,
        home_directory: PathBuf,
    ) -> Result<Self, SettingsError> {
        for (input, path) in [
            ("process_cwd", &process_cwd),
            ("home_directory", &home_directory),
        ] {
            if !path.is_absolute() {
                return Err(SettingsError::Location {
                    input,
                    origin: SettingsOrigin::Engine,
                });
            }
        }
        let working = anchored(
            working_directory.as_deref().unwrap_or(&process_cwd),
            &process_cwd,
            &home_directory,
        );
        let user = anchored(&user_root, &working, &home_directory);
        Ok(Self {
            working,
            user,
            home: home_directory,
        })
    }
    /// Trims path text, expands leading tilde at supplied home and resolves at cwd.
    /// Dot segments and separators normalize lexically, without filesystem lookup.
    pub fn invocation_path(&self, path: &Path) -> PathBuf {
        normalized(path, &self.working, &self.home)
    }

    /// Trims, expands tilde and lexically normalizes at the declaring directory.
    /// Declaring directories retain their supplied text; relative ones resolve at
    /// cwd. No filesystem lookup occurs.
    pub fn resource_path(&self, path: &Path, declaring_directory: &Path) -> PathBuf {
        let base = self.working.join(declaring_directory);
        normalized(path, &base, &self.home)
    }

    /// Effective working directory.
    pub fn working_directory(&self) -> &Path {
        &self.working
    }
    /// Selected user root or working-directory `.maestro` directory.
    pub fn configuration_directory(&self, scope: SettingsScope) -> PathBuf {
        match scope {
            SettingsScope::User => self.user.clone(),
            SettingsScope::Project => self.working.join(".maestro"),
        }
    }
}

fn anchored(path: &Path, base: &Path, home: &Path) -> PathBuf {
    if path == Path::new("~") {
        return home.to_owned();
    }
    if let Ok(rest) = path.strip_prefix("~/") {
        return home.join(rest);
    }
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn reference_whitespace(c: char) -> bool {
    c != '\u{85}' && (c.is_whitespace() || c == '\u{FEFF}')
}

#[cfg(unix)]
fn trimmed(path: &Path) -> &Path {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    let first = bytes.utf8_chunks().next().map_or("", |chunk| chunk.valid());
    let start = first.len() - first.trim_start_matches(reference_whitespace).len();
    let bytes = &bytes[start..];
    let last = bytes.utf8_chunks().last().map_or("", |chunk| {
        if chunk.invalid().is_empty() {
            chunk.valid()
        } else {
            ""
        }
    });
    let end = bytes.len() - (last.len() - last.trim_end_matches(reference_whitespace).len());
    Path::new(std::ffi::OsStr::from_bytes(&bytes[..end]))
}

#[cfg(not(unix))]
fn trimmed(path: &Path) -> &Path {
    path.to_str().map_or(path, |text| {
        Path::new(text.trim_matches(reference_whitespace))
    })
}

#[cfg(unix)]
fn tilde_suffix(path: &Path) -> Option<&Path> {
    use std::os::unix::ffi::OsStrExt;

    let rest = path.as_os_str().as_bytes().strip_prefix(b"~")?;
    let start = rest
        .iter()
        .position(|&byte| byte != b'/')
        .unwrap_or(rest.len());
    Some(Path::new(std::ffi::OsStr::from_bytes(&rest[start..])))
}

#[cfg(not(unix))]
fn tilde_suffix(path: &Path) -> Option<&Path> {
    let rest = path.to_str()?.strip_prefix('~')?;
    Some(Path::new(rest.trim_start_matches(std::path::is_separator)))
}

fn normalized(path: &Path, base: &Path, home: &Path) -> PathBuf {
    let path = trimmed(path);
    let anchored = if let Some(rest) = tilde_suffix(path) {
        home.join(rest)
    } else if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    let mut result = PathBuf::new();
    for component in anchored.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if matches!(
                    result.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) {
                    result.pop();
                } else if !result.has_root() {
                    result.push(component);
                }
            }
            _ => result.push(component),
        }
    }
    result
}
