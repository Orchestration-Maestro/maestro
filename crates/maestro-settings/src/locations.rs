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
    /// Relative declaring directories resolve at cwd; no filesystem lookup occurs.
    pub fn resource_path(&self, path: &Path, declaring_directory: &Path) -> PathBuf {
        let base = self.invocation_path(declaring_directory);
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

fn normalized(path: &Path, base: &Path, home: &Path) -> PathBuf {
    let path = path.to_str().map_or(path, |text| {
        Path::new(
            text.trim_matches(|c: char| c != '\u{85}' && (c.is_whitespace() || c == '\u{FEFF}')),
        )
    });
    let anchored = if let Some(rest) = path.to_str().and_then(|text| text.strip_prefix('~')) {
        home.join(rest.trim_start_matches(std::path::is_separator))
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
