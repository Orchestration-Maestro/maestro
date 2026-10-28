//! Ordered file preparation observed through both file-effect routes.

use crate::support::{self, Recorded};
use maestro_app::{ImageContent, ResizedImage};
use maestro_cli::input::file_processor::{
    FileArgumentError, FileArgumentOperations, NativeFileArgumentOperations, ProcessFileOptions,
    ProcessedFiles, process_file_arguments,
};
use maestro_test_directory::Scratch;
use std::cell::RefCell;
use std::io;

/// Environment variable naming the recorded case a child process replays.
const FIXTURE_CASE: &str = "MAESTRO_CLI_FIXTURE_CASE";

/// Copy the recorded file corpus into a disposable directory.
fn scratch() -> Scratch {
    let scratch = Scratch::new().unwrap();
    let source = format!("{}/tests/fixtures/files", env!("CARGO_MANIFEST_DIR"));
    copy_tree(std::path::Path::new(&source), scratch.path());
    scratch
}

/// Copy every file of `source` into `destination`.
fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
    }
    copy_quoted_name(destination);
}

/// Add the authored name a double quote makes legal only outside Windows.
#[cfg(unix)]
fn copy_quoted_name(destination: &std::path::Path) {
    std::fs::copy(
        destination.join("quote.txt"),
        destination.join("quote\".txt"),
    )
    .unwrap();
}

/// Windows cannot store a double quote in a name, so it has no such fixture.
#[cfg(not(unix))]
fn copy_quoted_name(_: &std::path::Path) {}

/// The authored file names of one case.
fn file_names(case: &Recorded) -> Vec<String> {
    case.input["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
}

/// The authored absolute paths of one case inside `root`.
fn authored(case: &Recorded, root: &std::path::Path) -> Vec<String> {
    file_names(case)
        .into_iter()
        .map(|name| format!("{}/{name}", root.display()))
        .collect()
}

/// The recorded fitting choice of one case.
fn options(case: &Recorded) -> ProcessFileOptions {
    ProcessFileOptions {
        auto_resize_images: case.input["options"]["autoResizeImages"].as_bool(),
    }
}

/// Rewrite one recorded wrapper text so it names the prepared directory.
fn expected_text(case: &Recorded, root: &std::path::Path) -> String {
    let mut text = case.expected["text"].as_str().unwrap().to_owned();
    for name in file_names(case) {
        text = text.replace(
            &format!("name=\"{name}\""),
            &format!("name=\"{}/{name}\"", root.display()),
        );
    }
    text
}

/// The recorded attachments as their shared records.
pub(crate) fn expected_images(case: &Recorded) -> Vec<ImageContent> {
    case.expected["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|image| ImageContent {
            data: image["data"].as_str().unwrap().to_owned(),
            mime_type: image["mimeType"].as_str().unwrap().to_owned(),
        })
        .collect()
}

/// Prepare one case's files through `operations` and compare the whole result.
fn assert_prepared(
    case: &Recorded,
    root: &std::path::Path,
    operations: &dyn FileArgumentOperations,
) {
    let files = authored(case, root);
    let processed = process_file_arguments(&files, options(case), operations).unwrap();
    assert_eq!(processed.text, expected_text(case, root), "{}", case.case);
    assert_eq!(processed.images, expected_images(case), "{}", case.case);
}

/// A supplied operation route that delegates to the native one and records its calls.
pub(crate) struct SuppliedFileOperations {
    /// The operation names in their call order.
    pub(crate) calls: RefCell<Vec<&'static str>>,
    /// The native route the supplied one delegates to.
    native: NativeFileArgumentOperations,
}

impl SuppliedFileOperations {
    /// A supplied route recording its calls with unstyled diagnostics.
    pub(crate) fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            native: NativeFileArgumentOperations::new(false),
        }
    }
}

impl FileArgumentOperations for SuppliedFileOperations {
    fn resolve_path(&self, authored: &str) -> io::Result<String> {
        self.calls.borrow_mut().push("resolve_path");
        self.native.resolve_path(authored)
    }

    fn access(&self, absolute: &str) -> io::Result<()> {
        self.calls.borrow_mut().push("access");
        self.native.access(absolute)
    }

    fn size(&self, absolute: &str) -> io::Result<u64> {
        self.calls.borrow_mut().push("size");
        self.native.size(absolute)
    }

    fn detect_image_mime_type(&self, absolute: &str) -> io::Result<Option<String>> {
        self.calls.borrow_mut().push("detect_image_mime_type");
        self.native.detect_image_mime_type(absolute)
    }

    fn read_file(&self, absolute: &str) -> io::Result<Vec<u8>> {
        self.calls.borrow_mut().push("read_file");
        self.native.read_file(absolute)
    }

    fn resize_image(&self, image: &ImageContent) -> Option<ResizedImage> {
        self.calls.borrow_mut().push("resize_image");
        self.native.resize_image(image)
    }

    fn fatal(&self, error: FileArgumentError) -> ! {
        self.native.fatal(error)
    }
}

/// The file effect one supplied route refuses.
#[derive(Clone, Copy)]
pub(crate) enum Refusal {
    /// Report the path length.
    Size,
    /// Report the supported image MIME type.
    Detect,
    /// Read an image the MIME type already accepted.
    Read,
    /// Read a file whose MIME type is not an image.
    TextRead,
}

/// A file-effect route that refuses one operation and delegates the rest.
pub(crate) struct RefusingOperations {
    /// The operation that refuses.
    refusal: Refusal,
    /// The operation names in their call order.
    pub(crate) calls: RefCell<Vec<&'static str>>,
    /// The native route the other operations delegate to.
    native: NativeFileArgumentOperations,
}

impl RefusingOperations {
    /// A supplied route refusing `refusal` with unstyled diagnostics.
    pub(crate) fn new(refusal: Refusal) -> Self {
        Self {
            refusal,
            calls: RefCell::new(Vec::new()),
            native: NativeFileArgumentOperations::new(false),
        }
    }

    /// The refusal every failing operation reports.
    fn refused() -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, "refused by the test route")
    }
}

impl FileArgumentOperations for RefusingOperations {
    fn resolve_path(&self, authored: &str) -> io::Result<String> {
        self.calls.borrow_mut().push("resolve_path");
        self.native.resolve_path(authored)
    }

    fn access(&self, absolute: &str) -> io::Result<()> {
        self.calls.borrow_mut().push("access");
        self.native.access(absolute)
    }

    fn size(&self, absolute: &str) -> io::Result<u64> {
        self.calls.borrow_mut().push("size");
        match self.refusal {
            Refusal::Size => Err(Self::refused()),
            _ => self.native.size(absolute),
        }
    }

    fn detect_image_mime_type(&self, absolute: &str) -> io::Result<Option<String>> {
        self.calls.borrow_mut().push("detect_image_mime_type");
        match self.refusal {
            Refusal::Detect => Err(Self::refused()),
            Refusal::TextRead => Ok(None),
            _ => self.native.detect_image_mime_type(absolute),
        }
    }

    fn read_file(&self, absolute: &str) -> io::Result<Vec<u8>> {
        self.calls.borrow_mut().push("read_file");
        match self.refusal {
            Refusal::Read | Refusal::TextRead => Err(Self::refused()),
            _ => self.native.read_file(absolute),
        }
    }

    fn resize_image(&self, image: &ImageContent) -> Option<ResizedImage> {
        self.native.resize_image(image)
    }

    fn fatal(&self, error: FileArgumentError) -> ! {
        self.native.fatal(error)
    }
}

#[test]
fn native_and_supplied_file_operations_have_the_same_contract() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    for case in support::operation("files", "files") {
        if case.expected.get("text").is_none() {
            // The fatal cases leave the process; they have their own witnesses.
            continue;
        }
        if cfg!(windows) && file_names(&case).iter().any(|name| name.contains('"')) {
            // Windows cannot store a double quote in a name, so it has no such fixture.
            continue;
        }
        assert_prepared(&case, scratch.path(), &native);
        let supplied = SuppliedFileOperations::new();
        assert_prepared(&case, scratch.path(), &supplied);
        let calls = supplied.calls.borrow().clone();
        assert!(calls.first() != Some(&"size"), "{}", case.case);
    }
}

#[test]
fn image_arguments_are_not_blocked_by_message_conversion_policy() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    let case = support::case_by_title("files", "files-194");
    let processed =
        process_file_arguments(&authored(&case, scratch.path()), options(&case), &native).unwrap();
    assert_eq!(processed.images.len(), 1);
    assert_eq!(processed.images[0].mime_type, "image/png");
    assert_eq!(processed.text, expected_text(&case, scratch.path()));
}

#[test]
fn text_arguments_keep_order_wrappers_and_empty_file_skips() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    for title in ["files-184", "files-185", "files-192"] {
        let case = support::case_by_title("files", title);
        assert_prepared(&case, scratch.path(), &native);
    }
}

#[test]
fn unfittable_images_leave_only_the_omission_wrapper() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    for title in ["files-188", "files-189", "files-193"] {
        let case = support::case_by_title("files", title);
        assert_prepared(&case, scratch.path(), &native);
    }
}

#[test]
fn default_fitting_and_disabled_passthrough_keep_separate_results() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    for title in ["files-182", "files-183", "files-190", "files-191"] {
        let case = support::case_by_title("files", title);
        assert_prepared(&case, scratch.path(), &native);
    }
}

#[test]
fn text_decoding_and_authored_file_names_are_not_rewritten() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    #[cfg(unix)]
    let titles = ["files-186", "files-187"];
    #[cfg(not(unix))]
    let titles = ["files-186"];
    for title in titles {
        let case = support::case_by_title("files", title);
        assert_prepared(&case, scratch.path(), &native);
    }
}

#[test]
fn read_path_variants_reach_the_existing_path_owner() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    let absolute = scratch.path().join("first.txt").display().to_string();
    let resolved = native.resolve_path(&absolute).unwrap();
    assert_eq!(resolved, absolute);
    for authored in ["./first.txt", "first.txt", "first.txt"] {
        let variant = native.resolve_path(authored).unwrap();
        let leaf = format!("{}first.txt", std::path::MAIN_SEPARATOR);
        assert!(variant.ends_with(&leaf), "{authored}");
    }
}

#[test]
fn file_failure_phases_keep_their_diagnostics_and_exit_status() {
    if std::env::var_os(FIXTURE_CASE).is_some() || std::env::var_os(FIXTURE_REFUSAL).is_some() {
        fatal_child();
    }
    for case in ["files-195", "files-196"] {
        let scratch = scratch();
        let output = maestro_test_process::TestProcess::current(
            "file_failure_phases_keep_their_diagnostics_and_exit_status",
        )
        .unwrap()
        .env(FIXTURE_CASE, case)
        .current_dir(scratch.path())
        .nocapture()
        .output()
        .unwrap();
        let recorded = support::case_by_title("files", case);
        assert_eq!(output.status.code(), Some(1), "{case}");
        let expected = recorded.expected["stderr"].as_str().unwrap().replace(
            "missing.txt",
            &scratch.path().join("missing.txt").display().to_string(),
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected,
            "the child's own diagnostic bytes for {case}"
        );
    }
}

#[test]
fn a_supplied_text_read_refusal_keeps_the_native_diagnostic_and_status() {
    if std::env::var_os(FIXTURE_CASE).is_some() || std::env::var_os(FIXTURE_REFUSAL).is_some() {
        fatal_child();
    }
    let scratch = scratch();
    let output = maestro_test_process::TestProcess::current(
        "a_supplied_text_read_refusal_keeps_the_native_diagnostic_and_status",
    )
    .unwrap()
    .env(FIXTURE_REFUSAL, "text-read")
    .current_dir(scratch.path())
    .nocapture()
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Error: Could not read file {}: refused by the test route\n",
            scratch.path().join("first.txt").display()
        ),
        "a refused text read reports its path and cause"
    );
}

#[test]
fn file_refusals_stop_at_the_stage_that_refused() {
    let scratch = scratch();
    let text = vec![format!("{}/first.txt", scratch.path().display())];
    let image = vec![format!("{}/tiny.png", scratch.path().display())];
    for (refusal, files, stage) in [
        (Refusal::Size, &text, "size"),
        (Refusal::Detect, &text, "detect_image_mime_type"),
        (Refusal::Read, &image, "read_file"),
    ] {
        let operations = RefusingOperations::new(refusal);
        let Err(error) = process_file_arguments(files, ProcessFileOptions::default(), &operations)
        else {
            panic!("{stage} stopped the preparation");
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{stage}");
        assert_eq!(
            operations.calls.borrow().last().copied(),
            Some(stage),
            "{stage} is the last effect the route was asked for"
        );
    }
}

/// Environment variable naming the supplied refusal a child replays.
const FIXTURE_REFUSAL: &str = "MAESTRO_CLI_FIXTURE_REFUSAL";

/// Run the recorded fatal case in this process, which leaves through the native exit.
fn fatal_child() {
    if std::env::var_os(FIXTURE_REFUSAL).is_some() {
        let files = vec!["first.txt".to_owned()];
        let operations = RefusingOperations::new(Refusal::TextRead);
        let prepared: io::Result<ProcessedFiles> =
            process_file_arguments(&files, ProcessFileOptions::default(), &operations);
        panic!(
            "the supplied fatal case returned a result: {}",
            prepared.is_ok()
        );
    }
    let case = std::env::var(FIXTURE_CASE).unwrap();
    let recorded = support::case_by_title("files", &case);
    let styles = recorded.input["styles_enabled"].as_bool().unwrap();
    let files = file_names(&recorded);
    let native = NativeFileArgumentOperations::new(styles);
    let options = ProcessFileOptions::default();
    let prepared: io::Result<ProcessedFiles> = process_file_arguments(&files, options, &native);
    panic!("the fatal case returned a result: {}", prepared.is_ok());
}
