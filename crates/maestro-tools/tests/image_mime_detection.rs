//! Native file sampling through the supported-image MIME operation.
#![cfg(not(target_arch = "wasm32"))]

use crate::read_support::Scratch;
use crate::support;
use support::TestResult;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use maestro_tools::utils::mime::detect_supported_image_mime_type_from_file;
use serde::Deserialize;
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};

/// File content query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Bytes independently recorded by the oracle.
    bytes: String,
}

/// Compare file-content decisions, never filename extensions.
fn check(name: &str) -> TestResult {
    for case in support::cases("image_mime_detection", name)? {
        assert_eq!(case.operation, "mime", "{}", case.id);
        let input: Input = serde_json::from_value(case.input)?;
        let bytes = STANDARD.decode(input.bytes)?;
        let directory = Scratch::new()?;
        let path = directory.path().join("misleading.txt");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(&bytes)?;
        let actual = detect_supported_image_mime_type_from_file(
            path.to_str().ok_or("non-UTF-8 scratch path")?,
        )?;
        assert_eq!(json!({"ok": actual}), case.expected, "{}", case.id);
    }
    Ok(())
}

#[test]
fn supported_mime_comes_from_file_bytes() -> TestResult {
    check("supported_mime_comes_from_file_bytes")
}

#[test]
fn png_sniff_distinguishes_animation_and_header_edges() -> TestResult {
    check("png_sniff_distinguishes_animation_and_header_edges")
}

#[test]
fn mime_sniff_retains_bom_and_id3_prefix_rules() -> TestResult {
    check("mime_sniff_retains_bom_and_id3_prefix_rules")
}

#[test]
fn mime_sniff_stops_at_4100_bytes() -> TestResult {
    check("mime_sniff_stops_at_4100_bytes")
}

/// An error-producing native path choice.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorInput {
    /// Missing file or owned directory.
    path: String,
}

/// Recorded native error class.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorOutput {
    /// Platform-independent oracle class.
    code: String,
}

#[test]
fn file_sniff_errors_keep_native_io_identity() -> TestResult {
    for case in support::cases(
        "image_mime_detection",
        "file_sniff_errors_keep_native_io_identity",
    )? {
        assert_eq!(case.operation, "file-error", "{}", case.id);
        let input: ErrorInput = serde_json::from_value(case.input)?;
        let expected: ErrorOutput = serde_json::from_value(case.expected)?;
        let directory = Scratch::new()?;
        let (path, kind) = match input.path.as_str() {
            "directory" => {
                assert_eq!(expected.code, "EISDIR");
                // Windows refuses to open a directory as a file.
                let kind = if cfg!(windows) {
                    std::io::ErrorKind::PermissionDenied
                } else {
                    std::io::ErrorKind::IsADirectory
                };
                (directory.path().to_path_buf(), kind)
            }
            "nonexistent-file" => {
                assert_eq!(expected.code, "ENOENT");
                (
                    directory.path().join("absent"),
                    std::io::ErrorKind::NotFound,
                )
            }
            _ => return Err("unknown error path".into()),
        };
        let actual = detect_supported_image_mime_type_from_file(
            path.to_str().ok_or("non-UTF-8 scratch path")?,
        )
        .expect_err("native error");
        let native = File::open(&path)
            .and_then(|mut file| file.read(&mut [0; 4100]))
            .expect_err("native operation");
        assert_eq!(actual.kind(), kind, "{}", case.id);
        assert_eq!(actual.raw_os_error(), native.raw_os_error(), "{}", case.id);
    }
    Ok(())
}
