//! Host operations consumed by filesystem completion.

use std::cmp::Ordering;
use std::io;

/// A directory entry with only the distinctions used by completion.
pub struct DirectoryEntry {
    /// Display spelling of the filename.
    pub name: String,
    /// Entry classification before following symbolic links.
    pub kind: DirectoryEntryKind,
}

/// Entry classifications used to decide directory candidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryEntryKind {
    /// A directory.
    Directory,
    /// A symbolic link requiring a separate metadata read.
    SymbolicLink,
    /// Any other entry.
    Other,
}

/// Replaceable filesystem, collation and search effects.
pub trait AutocompleteOperations {
    /// The caller's cancellation signal type.
    type Signal: ?Sized;
    /// Read request cancellation without changing the signal.
    fn is_aborted(&self, signal: &Self::Signal) -> bool;
    /// Run the supplied search executable and return successful stdout bytes.
    ///
    /// # Errors
    /// Returns process, pipe or cancellation failure.
    fn run_fd<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        signal: &'a Self::Signal,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<Vec<u8>>> + 'a>>;
    /// Read the native home directory.
    ///
    /// # Errors
    /// Returns the host home-lookup failure.
    fn home_dir(&self) -> io::Result<String>;
    /// Enumerate a supplied authored directory.
    ///
    /// # Errors
    /// Returns an enumeration failure.
    fn read_dir(&self, path: &str) -> io::Result<Vec<DirectoryEntry>>;
    /// Follow a supplied path to determine whether it is a directory.
    ///
    /// # Errors
    /// Returns a metadata-read failure.
    fn is_directory(&self, path: &str) -> io::Result<bool>;
    /// Compare labels within one directory class.
    ///
    /// # Errors
    /// Returns a collation failure.
    fn compare(&self, left: &str, right: &str) -> io::Result<Ordering>;
}

/// Native filesystem effects with a lazily retained locale collator.
///
/// Polling an attachment search requires an entered Tokio runtime with I/O
/// enabled. Without it, native process spawning or pipe registration can panic;
/// the provider's caught-error path handles I/O errors, not runtime panics.
/// Other completion operations do not require a runtime.
///
/// ```
/// use maestro_cancellation::Cancellation;
/// use maestro_tui::autocomplete::{
///     CompletionOptions, CursorPosition, NativeAutocompleteOperations,
/// };
/// use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider};
///
/// let runtime = tokio::runtime::Builder::new_current_thread()
///     .enable_io()
///     .build()?;
/// runtime.block_on(async {
///     let provider = CombinedAutocompleteProvider::new(
///         vec![], ".".into(), Some("fd".into()),
///         NativeAutocompleteOperations::default(),
///     );
///     let lines = vec!["@".into()];
///     let signal = Cancellation::new();
///     let suggestions = provider.get_suggestions(
///         &lines,
///         CursorPosition { line: 0, col: 1 },
///         CompletionOptions { signal: &signal, force: None },
///     ).await?;
///     // An unavailable executable yields no attachment suggestions.
///     if let Some(suggestions) = suggestions {
///         assert!(!suggestions.items.is_empty());
///     }
///     Ok::<(), Box<dyn std::error::Error>>(())
/// })?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeAutocompleteOperations<E = fn(&str) -> Option<String>> {
    /// Locale environment reader, independent of home lookup.
    environment: E,
    /// Collator selected on the first same-class label comparison.
    collator: std::cell::OnceCell<icu_collator::CollatorBorrowed<'static>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeAutocompleteOperations {
    fn default() -> Self {
        Self::with_environment(|name| std::env::var(name).ok())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<E: Fn(&str) -> Option<String>> NativeAutocompleteOperations<E> {
    /// Substitute locale environment lookup without changing filesystem or home effects.
    pub fn with_environment(environment: E) -> Self {
        Self {
            environment,
            collator: std::cell::OnceCell::new(),
        }
    }

    /// Prepare a locale collator from the first present environment operand.
    fn make_collator(&self) -> io::Result<icu_collator::CollatorBorrowed<'static>> {
        let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(&self.environment)
            .unwrap_or_else(|| "en_US".into());
        let base = raw.split(['.', '@']).next().unwrap_or_default();
        let locale = match base {
            "C" | "POSIX" => "en-US".into(),
            "" => "und".into(),
            value => value.replace('_', "-"),
        };
        let preferences = locale
            .parse::<icu_locale_core::Locale>()
            .map(Into::into)
            .unwrap_or_default();
        icu_collator::Collator::try_new(
            preferences,
            icu_collator::options::CollatorOptions::default(),
        )
        .map_err(io::Error::other)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<E: Fn(&str) -> Option<String>> AutocompleteOperations for NativeAutocompleteOperations<E> {
    type Signal = maestro_cancellation::Cancellation;

    fn is_aborted(&self, signal: &Self::Signal) -> bool {
        signal.is_aborted()
    }

    /// Run native search within the caller's entered, I/O-enabled Tokio runtime.
    ///
    /// # Panics
    /// Can panic when polled without an entered Tokio runtime with I/O enabled.
    fn run_fd<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        signal: &'a Self::Signal,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<Vec<u8>>> + 'a>> {
        Box::pin(super::fd::run(executable, args, signal))
    }

    fn home_dir(&self) -> io::Result<String> {
        std::env::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .ok_or_else(|| io::Error::other("home directory unavailable"))
    }

    fn read_dir(&self, path: &str) -> io::Result<Vec<DirectoryEntry>> {
        let mut entries = std::fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        entries
            .into_iter()
            .map(|entry| {
                let kind = entry.file_type()?;
                Ok(DirectoryEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    kind: if kind.is_dir() {
                        DirectoryEntryKind::Directory
                    } else if kind.is_symlink() {
                        DirectoryEntryKind::SymbolicLink
                    } else {
                        DirectoryEntryKind::Other
                    },
                })
            })
            .collect()
    }

    fn is_directory(&self, path: &str) -> io::Result<bool> {
        std::fs::metadata(path).map(|metadata| metadata.is_dir())
    }

    fn compare(&self, left: &str, right: &str) -> io::Result<Ordering> {
        if let Some(collator) = self.collator.get() {
            return Ok(collator.compare(left, right));
        }
        let collator = self.make_collator()?;
        let result = collator.compare(left, right);
        let _ = self.collator.set(collator);
        Ok(result)
    }
}
