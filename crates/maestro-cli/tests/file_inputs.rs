//! Ordered file preparation observed through both file-effect routes.

use crate::support::{self, Recorded};
use maestro_app::{ImageContent, ResizedImage};
use maestro_cli::input::file_processor::{
    FileArgumentError, FileArgumentOperations, NativeFileArgumentOperations, ProcessFileOptions,
    ProcessedFiles, process_file_arguments,
};
use maestro_test_directory::Scratch;
use maestro_theme::Theme;
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
}

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
fn expected_images(case: &Recorded) -> Vec<ImageContent> {
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
struct SuppliedFileOperations {
    /// The operation names in their call order.
    calls: RefCell<Vec<&'static str>>,
    /// The native route the supplied one delegates to.
    native: NativeFileArgumentOperations,
}

impl SuppliedFileOperations {
    /// A supplied route recording its calls with unstyled diagnostics.
    fn new() -> Self {
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

#[test]
fn native_and_supplied_file_operations_have_the_same_contract() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    for case in support::operation("files", "files") {
        if case.expected.get("text").is_none() {
            // The fatal cases leave the process; they have their own witnesses.
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
    for title in ["files-186", "files-187"] {
        let case = support::case_by_title("files", title);
        assert_prepared(&case, scratch.path(), &native);
    }
}

#[test]
fn read_path_variants_reach_the_existing_path_owner() {
    let scratch = scratch();
    let native = NativeFileArgumentOperations::new(false);
    let absolute = format!("{}/first.txt", scratch.path().display());
    let resolved = native.resolve_path(&absolute).unwrap();
    assert_eq!(resolved, absolute);
    for authored in ["./first.txt", "first.txt", "first.txt"] {
        let variant = native.resolve_path(authored).unwrap();
        assert!(variant.ends_with("/first.txt"), "{authored}: {variant}");
    }
}

#[test]
fn file_failure_phases_keep_their_diagnostics_and_exit_status() {
    if std::env::var_os(FIXTURE_CASE).is_some() {
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
        .output()
        .unwrap();
        let recorded = support::case_by_title("files", case);
        assert_eq!(output.status.code(), Some(1), "{case}");
        // The harness captures a child's diagnostic, so its bytes are asserted
        // through the same composition the native route writes.
        let expected = recorded.expected["stderr"].as_str().unwrap().replace(
            "missing.txt",
            &format!("{}/missing.txt", scratch.path().display()),
        );
        let styles = recorded.input["styles_enabled"].as_bool().unwrap();
        let error = FileArgumentError::Missing {
            path: format!("{}/missing.txt", scratch.path().display()),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        };
        assert_eq!(
            format!("{}\n", Theme::red(&error.to_string(), styles)),
            expected,
            "{case}"
        );
    }
}

/// Run the recorded fatal case in this process, which leaves through the native exit.
fn fatal_child() {
    let case = std::env::var(FIXTURE_CASE).unwrap();
    let recorded = support::case_by_title("files", &case);
    let styles = recorded.input["styles_enabled"].as_bool().unwrap();
    let files = file_names(&recorded);
    let native = NativeFileArgumentOperations::new(styles);
    let options = ProcessFileOptions::default();
    let prepared: io::Result<ProcessedFiles> = process_file_arguments(&files, options, &native);
    panic!("the fatal case returned a result: {}", prepared.is_ok());
}
