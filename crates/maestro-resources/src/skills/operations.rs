//! Filesystem observations consumed by resource discovery.

/// One filesystem directory entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Dirent {
    /// Declared name or directory basename.
    pub name: String,
    /// Whether the entry is a regular file.
    pub is_file: bool,
    /// Whether the entry is a directory.
    pub is_directory: bool,
    /// Whether the entry is a symbolic link.
    pub is_symbolic_link: bool,
}

/// File kind observed after following links.
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// Whether the entry is a regular file.
    pub is_file: bool,
    /// Whether the entry is a directory.
    pub is_directory: bool,
}

use crate::ResourceError;

/// Supplies filesystem effects; each failure retains its original message.
pub trait ResourceOperations {
    /// Tests existence, suppressing filesystem errors.
    fn exists(&self, path: &str) -> bool;
    /// Returns entries in the adapter's observed order.
    ///
    /// # Errors
    /// Returns the adapter failure when the operation cannot be completed.
    fn read_dir(&self, path: &str) -> Result<Vec<Dirent>, ResourceError>;
    /// Reads text with replacement for invalid UTF-8.
    ///
    /// # Errors
    /// Returns the adapter failure when the operation cannot be completed.
    fn read_file(&self, path: &str) -> Result<String, ResourceError>;
    /// Follows links to inspect a path.
    ///
    /// # Errors
    /// Returns the adapter failure when the operation cannot be completed.
    fn stat(&self, path: &str) -> Result<Stats, ResourceError>;
    /// Resolves filesystem aliases.
    ///
    /// # Errors
    /// Returns the adapter failure when the operation cannot be completed.
    fn realpath(&self, path: &str) -> Result<String, ResourceError>;
    /// Returns the process working directory.
    ///
    /// # Errors
    /// Returns the adapter failure when the operation cannot be completed.
    fn current_dir(&self) -> Result<String, ResourceError>;
}
/// Native filesystem adapter, unavailable in browser builds.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeResourceOperations;
#[cfg(not(target_arch = "wasm32"))]
impl ResourceOperations for NativeResourceOperations {
    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    fn read_dir(&self, path: &str) -> Result<Vec<Dirent>, ResourceError> {
        let entries = std::fs::read_dir(path)
            .map_err(|error| native_error(&error))?
            .map(|entry| {
                let entry = entry.map_err(|error| native_error(&error))?;
                let kind = entry.file_type().map_err(|error| native_error(&error))?;
                Ok(Dirent {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_file: kind.is_file(),
                    is_directory: kind.is_dir(),
                    is_symbolic_link: kind.is_symlink(),
                })
            })
            .collect::<Result<Vec<_>, ResourceError>>()?;
        Ok(entries)
    }
    fn read_file(&self, path: &str) -> Result<String, ResourceError> {
        std::fs::read(path)
            .map(|v| String::from_utf8_lossy(&v).into_owned())
            .map_err(|error| native_error(&error))
    }
    fn stat(&self, path: &str) -> Result<Stats, ResourceError> {
        std::fs::metadata(path)
            .map(|s| Stats {
                is_file: s.is_file(),
                is_directory: s.is_dir(),
            })
            .map_err(|error| native_error(&error))
    }
    fn realpath(&self, path: &str) -> Result<String, ResourceError> {
        std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|error| native_error(&error))
    }
    fn current_dir(&self) -> Result<String, ResourceError> {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| ResourceError {
                message: Some(e.to_string()),
            })
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn native_error(error: &std::io::Error) -> ResourceError {
    ResourceError {
        message: Some(error.to_string()),
    }
}
