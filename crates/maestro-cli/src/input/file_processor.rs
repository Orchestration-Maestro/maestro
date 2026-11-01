//! Ordered preparation of the authored file arguments.

use maestro_app::{
    ImageContent, ImageResizeOptions, ResizedImage, detect_supported_image_mime_type_from_file,
    format_dimension_note, lossy_string, resize_image, resolve_read_path,
};
use maestro_theme::Theme;
use std::error::Error;
use std::fmt;
use std::io;

/// The prepared text and image attachments of one authored file list.
pub struct ProcessedFiles {
    /// Wrapper text in its authored order.
    pub text: String,
    /// Image attachments in their authored order.
    pub images: Vec<ImageContent>,
}

/// The caller's image-fitting choice.
#[derive(Clone, Copy, Default)]
pub struct ProcessFileOptions {
    /// Whether images are fitted to the inline limit, absent meaning yes.
    pub auto_resize_images: Option<bool>,
}

/// A file input that stops the operation with its own diagnostic.
#[derive(Debug)]
pub enum FileArgumentError {
    /// The resolved path was not reachable.
    Missing {
        /// The resolved absolute path.
        path: String,
        /// The refusal the access check reported.
        source: io::Error,
    },
    /// A text file could not be read.
    TextRead {
        /// The resolved absolute path.
        path: String,
        /// The refusal the read reported.
        source: io::Error,
    },
}

impl fmt::Display for FileArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path, .. } => write!(formatter, "Error: File not found: {path}"),
            Self::TextRead { path, source } => {
                write!(formatter, "Error: Could not read file {path}: {source}")
            }
        }
    }
}

impl Error for FileArgumentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Missing { source, .. } | Self::TextRead { source, .. } => Some(source),
        }
    }
}

/// The effects one file preparation run needs.
pub trait FileArgumentOperations {
    /// Resolve one authored path to its absolute spelling.
    ///
    /// # Errors
    /// Returns the native path resolution error.
    fn resolve_path(&self, authored: &str) -> io::Result<String>;
    /// Confirm the resolved path is reachable.
    ///
    /// # Errors
    /// Returns the native refusal of the access check.
    fn access(&self, absolute: &str) -> io::Result<()>;
    /// Report the byte length of the resolved path.
    ///
    /// # Errors
    /// Returns the native metadata error.
    fn size(&self, absolute: &str) -> io::Result<u64>;
    /// Report the supported image MIME type of the file content.
    ///
    /// # Errors
    /// Returns the native read error of the content sample.
    fn detect_image_mime_type(&self, absolute: &str) -> io::Result<Option<String>>;
    /// Read the whole file.
    ///
    /// # Errors
    /// Returns the native read error.
    fn read_file(&self, absolute: &str) -> io::Result<Vec<u8>>;
    /// Fit one image to the inline limits.
    fn resize_image(&self, image: &ImageContent) -> Option<ResizedImage>;
    /// Report one fatal file input and stop.
    fn fatal(&self, error: FileArgumentError) -> !;
}

/// The file wrapper of one text file.
fn text_wrapper(path: &str, content: &str) -> String {
    format!("<file name=\"{path}\">\n{content}\n</file>\n")
}

/// The file wrapper of one image, carrying its dimension note when it has one.
fn image_wrapper(path: &str, note: Option<String>) -> String {
    format!(
        "<file name=\"{path}\">{}</file>\n",
        note.unwrap_or_default()
    )
}

/// The wrapper of an image the fitting could not bring under the limit.
fn omission_wrapper(path: &str) -> String {
    format!(
        "<file name=\"{path}\">[Image omitted: could not be resized below the inline image size limit.]</file>\n"
    )
}

/// Prepare one image argument, returning its attachment and wrapper.
fn prepare_image(
    absolute: &str,
    mime_type: &str,
    operations: &dyn FileArgumentOperations,
    auto_resize: bool,
) -> io::Result<Option<(ImageContent, String)>> {
    let content = operations.read_file(absolute)?;
    let data = base64_encode(&content);
    if !auto_resize {
        let attachment = ImageContent {
            data,
            mime_type: mime_type.to_owned(),
        };
        return Ok(Some((attachment, image_wrapper(absolute, None))));
    }
    let source = ImageContent {
        data,
        mime_type: mime_type.to_owned(),
    };
    let Some(fitted) = operations.resize_image(&source) else {
        return Ok(None);
    };
    let note = format_dimension_note(&fitted);
    let attachment = ImageContent {
        data: fitted.data,
        mime_type: fitted.mime_type,
    };
    Ok(Some((attachment, image_wrapper(absolute, note))))
}

/// Prepare one file argument, returning its text and attachment when it has one.
fn prepare_file(
    file_arg: &str,
    operations: &dyn FileArgumentOperations,
    auto_resize: bool,
) -> io::Result<Option<(String, Option<ImageContent>)>> {
    let absolute = operations.resolve_path(file_arg)?;
    if let Err(source) = operations.access(&absolute) {
        operations.fatal(FileArgumentError::Missing {
            path: absolute,
            source,
        });
    }
    if operations.size(&absolute)? == 0 {
        return Ok(None);
    }
    let Some(mime_type) = operations.detect_image_mime_type(&absolute)? else {
        return match operations.read_file(&absolute) {
            Ok(content) => Ok(Some((
                text_wrapper(&absolute, &lossy_string(content)),
                None,
            ))),
            Err(source) => operations.fatal(FileArgumentError::TextRead {
                path: absolute,
                source,
            }),
        };
    };
    match prepare_image(&absolute, &mime_type, operations, auto_resize)? {
        Some((attachment, text)) => Ok(Some((text, Some(attachment)))),
        None => Ok(Some((omission_wrapper(&absolute), None))),
    }
}

/// Process every authored file argument in its authored order.
///
/// # Errors
/// Returns the native stat, content-sample or image-read error of one argument.
pub fn process_file_arguments(
    file_args: &[String],
    options: ProcessFileOptions,
    operations: &dyn FileArgumentOperations,
) -> io::Result<ProcessedFiles> {
    let auto_resize = options.auto_resize_images.unwrap_or(true);
    let mut text = String::new();
    let mut images = Vec::new();
    for file_arg in file_args {
        let Some((wrapper, attachment)) = prepare_file(file_arg, operations, auto_resize)? else {
            continue;
        };
        text.push_str(&wrapper);
        images.extend(attachment);
    }
    Ok(ProcessedFiles { text, images })
}

/// Encode file content as base64.
fn base64_encode(content: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(content)
}

/// The native file effects of the running process.
pub struct NativeFileArgumentOperations {
    /// Whether diagnostics carry their visible style.
    styles_enabled: bool,
}

impl NativeFileArgumentOperations {
    /// Retain the supplied diagnostic-style choice.
    #[must_use]
    pub fn new(styles_enabled: bool) -> Self {
        Self { styles_enabled }
    }

    /// Report one fatal file input on the standard error stream and exit.
    fn report(&self, error: &FileArgumentError) -> ! {
        let message = Theme::red(&error.to_string(), self.styles_enabled);
        eprintln!("{message}");
        std::process::exit(1);
    }
}

impl FileArgumentOperations for NativeFileArgumentOperations {
    fn resolve_path(&self, authored: &str) -> io::Result<String> {
        let cwd = std::env::current_dir()?;
        resolve_read_path(authored, &cwd.to_string_lossy())
    }

    fn access(&self, absolute: &str) -> io::Result<()> {
        std::fs::metadata(absolute).map(|_| ())
    }

    fn size(&self, absolute: &str) -> io::Result<u64> {
        std::fs::metadata(absolute).map(|metadata| metadata.len())
    }

    fn detect_image_mime_type(&self, absolute: &str) -> io::Result<Option<String>> {
        detect_supported_image_mime_type_from_file(absolute).map(|mime| mime.map(str::to_owned))
    }

    fn read_file(&self, absolute: &str) -> io::Result<Vec<u8>> {
        std::fs::read(absolute)
    }

    fn resize_image(&self, image: &ImageContent) -> Option<ResizedImage> {
        resize_image(image, ImageResizeOptions::default())
    }

    fn fatal(&self, error: FileArgumentError) -> ! {
        self.report(&error)
    }
}
