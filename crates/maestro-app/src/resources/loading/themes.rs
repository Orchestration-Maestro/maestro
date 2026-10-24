//! Theme files and directories with their loading diagnostics.
use super::{DefaultEventBus, DefaultResourceLoader, LoadThemesResult};
use maestro_path::join;
use maestro_resources::{
    DiagnosticType, ResourceDiagnostic, ResourceFileType, metadata, paths::resolve, read_entries,
};
use std::{io, path::Path, rc::Rc};

/// The file name suffix of a theme file, compared case-sensitively.
const THEME_FILE_SUFFIX: &str = ".json";

/// A warning about `path`.
fn warning(message: String, path: &str) -> ResourceDiagnostic {
    ResourceDiagnostic {
        r#type: DiagnosticType::Warning,
        message,
        path: Some(path.into()),
        collision: None,
    }
}

/// Loads one theme file, or records why it failed.
fn load_file(path: &str, result: &mut LoadThemesResult) {
    match maestro_theme::load_theme_from_path(path, None) {
        Ok(theme) => result.themes.push(Rc::new(theme)),
        Err(error) => result.diagnostics.push(warning(error.to_string(), path)),
    }
}

/// Loads every `.json` file of `dir`, following links to files, in name order.
fn load_dir(dir: &str, result: &mut LoadThemesResult) {
    if !Path::new(dir).exists() {
        return;
    }
    let entries = match read_entries(Path::new(dir)) {
        Ok(entries) => entries,
        Err(error) => return result.diagnostics.push(warning(error.to_string(), dir)),
    };
    for entry in entries {
        let name = entry.name.to_string_lossy();
        let path = join(&[dir, &name]);
        let is_file = match entry.file_type {
            ResourceFileType::Symlink => {
                metadata(Path::new(&path)).ok() == Some(ResourceFileType::File)
            }
            file_type => file_type == ResourceFileType::File,
        };
        if is_file && name.ends_with(THEME_FILE_SUFFIX) {
            load_file(&path, result);
        }
    }
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Loads each theme path: a directory's `.json` files or a `.json` file.
    pub(super) fn load_themes(&self, paths: &[String]) -> io::Result<LoadThemesResult> {
        let mut result = LoadThemesResult::default();
        for path in paths {
            let resolved = resolve(&[&self.cwd, path])?;
            if !Path::new(&resolved).exists() {
                result
                    .diagnostics
                    .push(warning("theme path does not exist".into(), &resolved));
                continue;
            }
            match metadata(Path::new(&resolved)) {
                Ok(ResourceFileType::Directory) => load_dir(&resolved, &mut result),
                Ok(ResourceFileType::File) if resolved.ends_with(THEME_FILE_SUFFIX) => {
                    load_file(&resolved, &mut result);
                }
                Ok(_) => result
                    .diagnostics
                    .push(warning("theme path is not a json file".into(), &resolved)),
                Err(error) => result
                    .diagnostics
                    .push(warning(error.to_string(), &resolved)),
            }
        }
        Ok(result)
    }
}
