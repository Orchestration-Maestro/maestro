//! Behavior of the read tool's execution through controlled and native operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::read_support;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::ImageOutputFormat;
use maestro_models::{Cancellation, DiagnosticCode, ModelInput};
use maestro_tools::{ReadOperations, ReadToolInput, ReadToolOptions, create_read_tool_definition};
use read_support::*;
use serde_json::{Value, json};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

/// A one-pixel PNG.
const TINY_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";
/// The text the model sees when fitting fails.
const OMITTED: &str = "Read image file [image/jpeg]\n[Image omitted: could not be resized below the inline image size limit.]";
/// The text the model sees beside images it cannot accept.
const NON_VISION: &str =
    "[Current model does not support images. The image will be omitted from this request.]";

/// The expected JSON of an attached image block.
fn image_json(data: &str, mime: &str) -> Value {
    json!({"type": "image", "data": data, "mimeType": mime})
}

/// The fixed lines `Line 1` through `Line n`, joined by newlines.
fn numbered(n: usize) -> String {
    (1..=n)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a read over `bytes` that the scripted detector reports as `mime`, with options and an optional model.
fn read_image(
    bytes: &[u8],
    mime: &str,
    auto_resize: Option<bool>,
    model: Option<Vec<ModelInput>>,
) -> Value {
    let ops = Arc::new(ScriptedOps::new(bytes).with_mime(mime));
    let tool = create_read_tool_definition::<SharedModel>(
        "/work".into(),
        ReadToolOptions {
            auto_resize_images: auto_resize,
            operations: Some(Arc::clone(&ops) as Arc<dyn ReadOperations>),
        },
    );
    let context = SharedModel(Arc::new(RwLock::new(model.map(self::model))));
    let result = call(&tool, input("/work/i", None, None), None, Some(context)).unwrap();
    assert!(result.details.is_none());
    content_json(&result)
}

#[test]
fn read_declaration_has_exact_schema_and_prompt_text() {
    let tool = create_read_tool_definition::<()>("/work".into(), ReadToolOptions::default());
    assert_eq!(tool.definition.name, "read");
    assert_eq!(tool.label, "read");
    assert_eq!(
        tool.definition.description,
        "Read the contents of a file. Supports text files and images (jpg, png, gif, webp). Images are sent as attachments. For text files, output is truncated to 2000 lines or 50KB (whichever is hit first). Use offset/limit for large files. When you need the full file, continue with offset until complete."
    );
    assert_eq!(tool.prompt_snippet.as_deref(), Some("Read file contents"));
    assert_eq!(
        tool.prompt_guidelines,
        Some(vec![
            "Use read to examine files instead of cat or sed.".to_owned()
        ])
    );
    let expected = json!({
        "type": "object",
        "required": ["path"],
        "properties": {
            "path": {"type": "string", "description": "Path to the file to read (relative or absolute)"},
            "offset": {"type": "number", "description": "Line number to start reading from (1-indexed)"},
            "limit": {"type": "number", "description": "Maximum number of lines to read"},
        },
    });
    assert_eq!(*tool.definition.parameters, expected);
    let keys: Vec<_> = tool.definition.parameters["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(keys, ["path", "offset", "limit"]);
}

#[test]
fn read_preserves_complete_text_and_absent_details() {
    let ops = Arc::new(ScriptedOps::new(b"Hello, world!\nLine 2\nLine 3"));
    let result = call(&tool_over(&ops), input("/work/f", None, None), None, None).unwrap();
    assert_eq!(
        content_json(&result),
        json!([text_json("Hello, world!\nLine 2\nLine 3")])
    );
    assert!(result.details.is_none());
    assert!(result.terminate.is_none());
}

#[test]
fn read_applies_offset_and_limit_before_truncation() {
    let file = numbered(100);
    let tail = (51..=100)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(read_text(file.as_bytes(), Some(51.0), None).unwrap(), tail);
    assert_eq!(
        read_text(file.as_bytes(), None, Some(10.0)).unwrap(),
        format!(
            "{}\n\n[90 more lines in file. Use offset=11 to continue.]",
            numbered(10)
        )
    );
    let window = (41..=60)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        read_text(file.as_bytes(), Some(41.0), Some(20.0)).unwrap(),
        format!("{window}\n\n[40 more lines in file. Use offset=61 to continue.]")
    );
    assert_eq!(read_text(file.as_bytes(), None, Some(100.0)).unwrap(), file);
    assert_eq!(
        read_text(file.as_bytes(), Some(101.0), None).unwrap_err(),
        "Offset 101 is beyond end of file (100 lines total)"
    );
}

/// An offset, a limit and the output or error they must produce.
type Slice = (Option<f64>, Option<f64>, Result<String, String>);

#[test]
fn read_numeric_slices_match_finite_number_rules() {
    let more = |n: &str, next: &str| {
        format!("\n\n[{n} more lines in file. Use offset={next} to continue.]")
    };
    let all = "a\nb\nc\nd".to_owned();
    let cases: Vec<Slice> = vec![
        (None, None, Ok(all.clone())),
        (Some(0.0), None, Ok(all.clone())),
        (Some(-2.0), None, Ok(all.clone())),
        (Some(0.5), None, Ok(all.clone())),
        (Some(1.5), None, Ok(all.clone())),
        (Some(2.9), None, Ok("b\nc\nd".into())),
        (Some(4.0), None, Ok("d".into())),
        (Some(4.9), None, Ok("d".into())),
        (
            Some(5.0),
            None,
            Err("Offset 5 is beyond end of file (4 lines total)".into()),
        ),
        (None, Some(0.0), Ok(more("4", "1"))),
        (None, Some(-1.0), Ok(format!("a\nb\nc{}", more("5", "0")))),
        (None, Some(-5.0), Ok(more("9", "-4"))),
        (None, Some(0.5), Ok(more("3.5", "1.5"))),
        (None, Some(1.5), Ok(format!("a{}", more("2.5", "2.5")))),
        (None, Some(10.0), Ok(all.clone())),
        (Some(2.5), Some(1.5), Ok(format!("b\nc{}", more("1", "4")))),
        (Some(2.0), Some(-1.0), Ok(more("4", "1"))),
        (Some(3.5), Some(-2.0), Ok(more("3.5", "1.5"))),
        (
            Some(1e20),
            None,
            Err("Offset 100000000000000000000 is beyond end of file (4 lines total)".into()),
        ),
        (None, Some(1e20), Ok(all)),
    ];
    assert_eq!(cases.len(), 20);
    for (index, (offset, limit, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            read_text(b"a\nb\nc\nd", offset, limit),
            expected,
            "case {index}"
        );
    }
}

#[test]
fn read_keeps_empty_lines_bom_and_lossy_utf8() {
    assert_eq!(read_text(b"", None, None).unwrap(), "");
    assert_eq!(read_text(b"a\nb\n", Some(3.0), None).unwrap(), "");
    assert_eq!(
        read_text(b"a\nb\n", Some(4.0), None).unwrap_err(),
        "Offset 4 is beyond end of file (3 lines total)"
    );
    assert_eq!(read_text(b"a\r\nb\r\n", None, None).unwrap(), "a\r\nb\r\n");
    assert_eq!(
        read_text("\u{feff}a\nb".as_bytes(), None, None).unwrap(),
        "\u{feff}a\nb"
    );
    let invalid = [
        0xEF, 0xBB, 0xBF, b'a', 0xC0, 0xAF, 0xF0, 0x9F, 0x92, b'\n', 0, b'b',
    ];
    assert_eq!(
        read_text(&invalid, None, None).unwrap(),
        "\u{feff}a\u{fffd}\u{fffd}\u{fffd}\n\0b"
    );
}

#[test]
fn read_reports_line_and_byte_continuations() {
    let short = vec!["ab"; 2500].join("\n");
    let ops = Arc::new(ScriptedOps::new(short.as_bytes()));
    let result = call(&tool_over(&ops), input("/work/f", None, None), None, None).unwrap();
    let kept = vec!["ab"; 2000].join("\n");
    assert_eq!(
        only_text(&result),
        format!("{kept}\n\n[Showing lines 1-2000 of 2500. Use offset=2001 to continue.]")
    );
    assert_eq!(
        serde_json::to_value(result.details.unwrap()).unwrap(),
        json!({"truncation": {
            "content": kept, "truncated": true, "truncatedBy": "lines", "totalLines": 2500,
            "totalBytes": 7499, "outputLines": 2000, "outputBytes": 5999,
            "lastLinePartial": false, "firstLineExceedsLimit": false, "maxLines": 2000, "maxBytes": 51200
        }})
    );
    let wide = vec!["x".repeat(200); 400].join("\n");
    let ops = Arc::new(ScriptedOps::new(wide.as_bytes()));
    let result = call(&tool_over(&ops), input("/work/f", None, None), None, None).unwrap();
    let kept = vec!["x".repeat(200); 254].join("\n");
    assert_eq!(
        only_text(&result),
        format!(
            "{kept}\n\n[Showing lines 1-254 of 400 (50.0KB limit). Use offset=255 to continue.]"
        )
    );
    assert_eq!(
        serde_json::to_value(result.details.unwrap()).unwrap(),
        json!({"truncation": {
            "content": kept, "truncated": true, "truncatedBy": "bytes", "totalLines": 400,
            "totalBytes": 80399, "outputLines": 254, "outputBytes": 51053,
            "lastLinePartial": false, "firstLineExceedsLimit": false, "maxLines": 2000, "maxBytes": 51200
        }})
    );
}

/// The guidance for an oversized selected line.
fn guidance(line: &str, path: &str) -> String {
    format!(
        "[Line {line} is 50.0KB, exceeds 50.0KB limit. Use bash: sed -n '{line}p' {path} | head -c 51200]"
    )
}

#[test]
fn read_oversized_first_line_guidance_uses_selected_line() {
    let exact = "x".repeat(51200);
    assert_eq!(read_text(exact.as_bytes(), None, None).unwrap(), exact);
    let long = "x".repeat(51201);
    assert_eq!(
        read_text(long.as_bytes(), None, None).unwrap(),
        guidance("1", "/work/f")
    );
    let behind = format!("short\n{long}");
    assert_eq!(
        read_text(behind.as_bytes(), Some(2.0), None).unwrap(),
        guidance("2", "/work/f")
    );
    assert_eq!(
        read_text(long.as_bytes(), None, Some(0.0)).unwrap(),
        "\n\n[1 more lines in file. Use offset=1 to continue.]"
    );
    let capped = (0..2000)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(read_text(capped.as_bytes(), None, None).unwrap(), capped);
    let expanding = vec![0xFF; 17067];
    assert_eq!(
        read_text(&expanding, None, None).unwrap(),
        guidance("1", "/work/f")
    );
    let ops = Arc::new(ScriptedOps::new(long.as_bytes()));
    let result = call(&tool_over(&ops), input("/work/a b", None, None), None, None).unwrap();
    assert_eq!(only_text(&result), guidance("1", "/work/a b"));
    assert_eq!(
        serde_json::to_value(result.details.unwrap()).unwrap(),
        json!({"truncation": {
            "content": "", "truncated": true, "truncatedBy": "bytes", "totalLines": 1,
            "totalBytes": 51201, "outputLines": 0, "outputBytes": 0, "lastLinePartial": false,
            "firstLineExceedsLimit": true, "maxLines": 2000, "maxBytes": 51200
        }})
    );
}

#[test]
fn read_fractional_oversize_guidance_uses_effective_line() {
    let behind = format!("short\n{}", "x".repeat(51201));
    assert_eq!(
        read_text(behind.as_bytes(), Some(2.5), None).unwrap(),
        guidance("2", "/work/f")
    );
}

#[test]
fn read_forwards_access_detection_and_read_failures() {
    for (stage, calls) in [
        (Stage::Access, vec!["access:/work/f"]),
        (Stage::Mime, vec!["access:/work/f", "mime:/work/f"]),
        (
            Stage::Read,
            vec!["access:/work/f", "mime:/work/f", "read:/work/f"],
        ),
    ] {
        let ops = Arc::new(ScriptedOps::new(b"x").at(stage, Behavior::Fail, None));
        let error = call(&tool_over(&ops), input("/work/f", None, None), None, None)
            .err()
            .unwrap();
        assert_eq!(error, ScriptedOps::failure(stage));
        assert_eq!(ops.log(), calls);
    }
    let dir = Scratch::new().unwrap();
    let tool = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    let error = call(&tool, input("missing.txt", None, None), None, None)
        .err()
        .unwrap();
    assert_eq!(error.code, Some(DiagnosticCode::Number(2.0)));
}

#[test]
fn read_cancellation_stops_later_operations() {
    let before = Cancellation::new();
    before.abort();
    let ops = Arc::new(ScriptedOps::new(b"x"));
    let error = call(
        &tool_over(&ops),
        input("/work/f", None, None),
        Some(before),
        None,
    )
    .err()
    .unwrap();
    assert_eq!(error.message, "Operation aborted");
    assert!(ops.log().is_empty());
    for (stage, calls) in [(Stage::Access, 1), (Stage::Mime, 2), (Stage::Read, 3)] {
        let signal = Cancellation::new();
        let ops =
            Arc::new(ScriptedOps::new(b"x").at(stage, Behavior::AbortThenSucceed, Some(&signal)));
        let error = call(
            &tool_over(&ops),
            input("/work/f", None, None),
            Some(signal),
            None,
        )
        .err()
        .unwrap();
        assert_eq!(error.message, "Operation aborted", "{stage:?}");
        assert_eq!(ops.log().len(), calls, "{stage:?}");
    }
}

#[test]
fn read_cancellation_interrupts_pending_operations() {
    for (stage, calls) in [(Stage::Access, 1), (Stage::Mime, 2), (Stage::Read, 3)] {
        let signal = Cancellation::new();
        let ops = Arc::new(ScriptedOps::new(b"x").at(stage, Behavior::Pend, None));
        let tool = tool_over(&ops);
        let mut future = (tool.execute)(
            "call".into(),
            input("/work/f", None, None),
            Some(signal.clone()),
            None,
            None,
        );
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        assert!(future.as_mut().poll(&mut cx).is_pending(), "{stage:?}");
        assert_eq!(ops.polled.load(Ordering::SeqCst), 1, "{stage:?}");
        assert_eq!(ops.dropped.load(Ordering::SeqCst), 0, "{stage:?}");
        signal.abort();
        let std::task::Poll::Ready(Err(error)) = future.as_mut().poll(&mut cx) else {
            panic!("{stage:?}: read did not settle after the abort");
        };
        assert_eq!(error.message, "Operation aborted", "{stage:?}");
        assert_eq!(ops.dropped.load(Ordering::SeqCst), 1, "{stage:?}");
        assert_eq!(ops.log().len(), calls, "{stage:?}");
    }
}

#[test]
fn read_cancellation_during_conversion_beats_result_and_failure() {
    let png = STANDARD.decode(TINY_PNG).unwrap();
    for (bytes, mime, offset) in [
        (png.as_slice(), Some("image/png"), None),
        (b"one\ntwo".as_slice(), None, None),
        (b"one\ntwo".as_slice(), None, Some(9.0)),
    ] {
        let signal = Cancellation::new();
        let mut ops =
            ScriptedOps::new(bytes).at(Stage::Read, Behavior::AbortWhenDropped, Some(&signal));
        if let Some(mime) = mime {
            ops = ops.with_mime(mime);
        }
        let ops = Arc::new(ops);
        let error = call(
            &tool_over(&ops),
            input("/work/f", offset, None),
            Some(signal),
            None,
        )
        .err()
        .unwrap();
        assert_eq!(error.message, "Operation aborted", "{mime:?} {offset:?}");
    }
}

#[test]
fn read_context_is_observed_after_mime_detection() {
    let png = STANDARD.decode(TINY_PNG).unwrap();
    for (initial, after, note) in [
        (
            vec![ModelInput::Text],
            vec![ModelInput::Text, ModelInput::Image],
            false,
        ),
        (
            vec![ModelInput::Text, ModelInput::Image],
            vec![ModelInput::Text],
            true,
        ),
    ] {
        let slot = Arc::new(RwLock::new(Some(model(initial))));
        let (hook_slot, replacement) = (Arc::clone(&slot), model(after));
        let mut ops = ScriptedOps::new(&png).with_mime("image/png");
        ops.on_detect = Some(Box::new(move || {
            *hook_slot.write().unwrap() = Some(Arc::clone(&replacement));
        }));
        let options = ReadToolOptions {
            auto_resize_images: None,
            operations: Some(Arc::new(ops)),
        };
        let tool = create_read_tool_definition::<SharedModel>("/work".into(), options);
        let result = call(
            &tool,
            input("/work/i", None, None),
            None,
            Some(SharedModel(slot)),
        )
        .unwrap();
        let header = if note {
            format!("Read image file [image/png]\n{NON_VISION}")
        } else {
            "Read image file [image/png]".into()
        };
        assert_eq!(
            content_json(&result),
            json!([text_json(&header), image_json(TINY_PNG, "image/png")])
        );
    }
}

#[test]
fn read_nonvision_note_does_not_remove_the_attachment() {
    let png = STANDARD.decode(TINY_PNG).unwrap();
    let models: [(Option<Vec<ModelInput>>, bool); 4] = [
        (None, false),
        (Some(vec![ModelInput::Text]), true),
        (Some(vec![ModelInput::Text, ModelInput::Image]), false),
        (Some(vec![]), true),
    ];
    for (model, note) in models {
        for resize in [None, Some(false)] {
            let header = if note {
                format!("Read image file [image/png]\n{NON_VISION}")
            } else {
                "Read image file [image/png]".into()
            };
            let actual = read_image(&png, "image/png", resize, model.clone());
            assert_eq!(
                actual,
                json!([text_json(&header), image_json(TINY_PNG, "image/png")])
            );
        }
    }
    let ops = Arc::new(ScriptedOps::new(b"ordinary text"));
    let tool = create_read_tool_definition::<SharedModel>(
        "/work".into(),
        ReadToolOptions {
            auto_resize_images: None,
            operations: Some(ops),
        },
    );
    let context = SharedModel(Arc::new(RwLock::new(Some(model(vec![ModelInput::Text])))));
    let result = call(&tool, input("/work/f", None, None), None, Some(context)).unwrap();
    assert_eq!(only_text(&result), "ordinary text");
}

#[test]
fn read_unfittable_image_has_only_the_exact_notice() {
    let broken = [255, 216, 255];
    for (model, note) in [
        (None, false),
        (Some(vec![ModelInput::Text]), true),
        (Some(vec![ModelInput::Text, ModelInput::Image]), false),
        (Some(vec![]), true),
    ] {
        let text = if note {
            format!("{OMITTED}\n{NON_VISION}")
        } else {
            OMITTED.to_owned()
        };
        assert_eq!(
            read_image(&broken, "image/jpeg", None, model),
            json!([text_json(&text)])
        );
    }
}

#[test]
fn read_resize_default_and_bypass_keep_image_contract() {
    let wide = blank_image(2200, 20, ImageOutputFormat::Png);
    let wide64 = STANDARD.encode(&wide);
    let fitted = |options: Option<bool>| read_image(&wide, "image/png", options, None);
    let default = fitted(None);
    assert_eq!(default, fitted(Some(true)));
    assert_ne!(default[1]["data"], json!(wide64));
    assert_eq!(
        fitted(Some(false)),
        json!([
            text_json("Read image file [image/png]"),
            image_json(&wide64, "image/png")
        ])
    );
    for (format, mime) in [
        (ImageOutputFormat::Png, "image/png"),
        (ImageOutputFormat::Jpeg(80), "image/jpeg"),
        (ImageOutputFormat::Gif, "image/gif"),
    ] {
        let bytes = blank_image(3, 2, format);
        let expected = json!([
            text_json(&format!("Read image file [{mime}]")),
            image_json(&STANDARD.encode(&bytes), mime)
        ]);
        assert_eq!(read_image(&bytes, mime, None, None), expected);
        assert_eq!(read_image(&bytes, mime, Some(false), None), expected);
    }
    assert_eq!(
        read_image(&[255, 216, 255], "image/jpeg", Some(false), None),
        json!([
            text_json("Read image file [image/jpeg]"),
            image_json("/9j/", "image/jpeg")
        ])
    );
}

#[test]
fn read_resized_image_includes_coordinate_note() {
    let wide = blank_image(2200, 20, ImageOutputFormat::Png);
    let content = read_image(&wide, "image/png", None, None);
    assert_eq!(
        content[0],
        text_json(
            "Read image file [image/png]\n[Image: original 2200x20, displayed at 2000x18. Multiply coordinates by 1.10 to map to original image.]"
        )
    );
    assert_eq!(content[1]["mimeType"], "image/png");
    let shown = image::load_from_memory(
        &STANDARD
            .decode(content[1]["data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!((shown.width(), shown.height()), (2000, 18));
}

#[test]
fn read_supplied_mime_is_not_restricted_to_builtin_formats() {
    assert_eq!(
        read_image(&[1, 2, 3], "image/custom", Some(false), None),
        json!([
            text_json("Read image file [image/custom]"),
            image_json("AQID", "image/custom")
        ])
    );
}

#[test]
fn read_optional_detector_keeps_binary_text() {
    let png = STANDARD.decode(TINY_PNG).unwrap();
    let lossy = String::from_utf8_lossy(&png).into_owned();
    let tool = create_read_tool_definition::<()>(
        "/work".into(),
        ReadToolOptions {
            auto_resize_images: None,
            operations: Some(Arc::new(NoDetector(png.clone()))),
        },
    );
    assert_eq!(
        only_text(&call(&tool, input("/work/p", None, None), None, None).unwrap()),
        lossy
    );
    for mime in [None, Some("")] {
        let mut ops = ScriptedOps::new(&png);
        ops.mime = mime.map(str::to_owned);
        let result = call(
            &tool_over(&Arc::new(ops)),
            input("/work/p", None, None),
            None,
            None,
        )
        .unwrap();
        assert_eq!(only_text(&result), lossy);
    }
    let dir = Scratch::new().unwrap();
    dir.file("sniff.bin", &[66, 77, 0, 255]);
    let tool = create_read_tool_definition::<()>(dir.text(), ReadToolOptions::default());
    assert_eq!(
        only_text(&call(&tool, input("sniff.bin", None, None), None, None).unwrap()),
        "BM\0\u{fffd}"
    );
}

#[test]
fn read_uses_magic_and_preserves_image_order() {
    let dir = Scratch::new().unwrap();
    let png = STANDARD.decode(TINY_PNG).unwrap();
    dir.file("photo", &png);
    dir.file("fake.png", b"definitely not a png");
    dir.file(
        "sample.jpg",
        &[0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F'],
    );
    dir.file("sample.gif", b"GIF89a\x01\x00\x01\x00");
    dir.file("sample.webp", b"RIFF\x04\0\0\0WEBPVP8 ");
    let read = |options: ReadToolOptions, args: ReadToolInput| {
        let tool = create_read_tool_definition::<()>(dir.text(), options);
        content_json(&call(&tool, args, None, None).unwrap())
    };
    let keep = || ReadToolOptions {
        auto_resize_images: Some(false),
        operations: None,
    };
    assert_eq!(
        read(
            ReadToolOptions::default(),
            input("photo", Some(1e20), Some(0.0))
        ),
        json!([
            text_json("Read image file [image/png]"),
            image_json(TINY_PNG, "image/png")
        ])
    );
    assert_eq!(
        read(ReadToolOptions::default(), input("fake.png", None, None)),
        json!([text_json("definitely not a png")])
    );
    for (name, mime, bytes) in [
        (
            "sample.jpg",
            "image/jpeg",
            std::fs::read(dir.path().join("sample.jpg")).unwrap(),
        ),
        (
            "sample.gif",
            "image/gif",
            std::fs::read(dir.path().join("sample.gif")).unwrap(),
        ),
        (
            "sample.webp",
            "image/webp",
            std::fs::read(dir.path().join("sample.webp")).unwrap(),
        ),
    ] {
        assert_eq!(
            read(keep(), input(name, None, None)),
            json!([
                text_json(&format!("Read image file [{mime}]")),
                image_json(&STANDARD.encode(bytes), mime)
            ])
        );
    }
}

#[test]
fn read_image_resize_yields_to_other_runtime_tasks() {
    let wide = blank_image(2200, 20, ImageOutputFormat::Png);
    let ops = Arc::new(ScriptedOps::new(&wide).with_mime("image/png"));
    let tool = tool_over(&ops);
    let ran = Arc::new(AtomicBool::new(false));
    let ran_before_read_returned = crate::runtime::run(async {
        let flag = Arc::clone(&ran);
        let ticker = tokio::spawn(async move { flag.store(true, Ordering::SeqCst) });
        let result = (tool.execute)(
            "call".into(),
            input("/work/i", None, None),
            None,
            None,
            None,
        )
        .await;
        let seen = ran.load(Ordering::SeqCst);
        ticker.await.unwrap();
        assert_eq!(result.unwrap().content.len(), 2);
        seen
    });
    assert!(ran_before_read_returned);
}

#[test]
fn read_image_abort_settles_before_blocking_resize_finishes() {
    let png = blank_image(4096, 64, ImageOutputFormat::Png);
    let ops = Arc::new(ScriptedOps::new(&png).with_mime("image/png"));
    let tool = tool_over(&ops);
    let signal = Cancellation::new();
    let (started, waiting) = std::sync::mpsc::channel();
    let (release, finish) = std::sync::mpsc::channel::<()>();
    let finish = std::sync::Mutex::new(finish);
    let runtime = crate::runtime::builder()
        .on_thread_start(move || {
            started.send(()).unwrap();
            // Hold the admitted resize off the async thread until the assertion is observed.
            let _ = finish.lock().unwrap().recv();
        })
        .build()
        .unwrap();
    runtime.block_on(async move {
        let mut call = (tool.execute)(
            "call".into(),
            input("/work/image.png", None, None),
            Some(signal.clone()),
            None,
            None,
        );
        assert!(crate::write_support::poll_once(&mut call).is_pending());
        waiting.recv().unwrap();
        signal.abort();
        let settled = crate::write_support::poll_once(&mut call);
        drop(release);
        let std::task::Poll::Ready(Err(error)) = settled else {
            panic!("read must settle before the blocking resize can finish");
        };
        assert_eq!(error.message, "Operation aborted");
        tokio::task::yield_now().await;
    });
}
