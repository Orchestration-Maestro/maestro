#![cfg(test)]
#![cfg(unix)]

use crate::toolchain_support;

use crate::recipe_support;
use recipe_support::Workspace;

/// Failed setup commands retain Cargo diagnostics in both output destinations.
#[test]
fn browser_setup_errors_reach_stderr_and_transcript() {
    if let Ok(stage) = std::env::var("MAESTRO_SETUP_FAILURE") {
        if stage == "recipe" {
            let _workspace = Workspace::new();
        } else {
            let browser = Browser::new();
            fs::write(
                browser.fixture("Cargo.toml"),
                "invalid_manifest_marker = [\n",
            )
            .unwrap();
            if stage == "lockfile" {
                browser.generate_lockfile();
            } else {
                browser_support::cold_sources(&browser);
            }
        }
        panic!("setup must fail");
    }
    let mut failures = Vec::new();
    for stage in ["recipe", "lockfile", "vendor"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/browser-author-diagnostics/setup-{}-{stage}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let child = maestro_test_process::TestProcess::current(
            "browser_setup_errors_reach_stderr_and_transcript",
        )
        .unwrap()
        .nocapture()
        .env("MAESTRO_SETUP_FAILURE", stage)
        .env("MAESTRO_TEST_DIAGNOSTICS_DIR", &directory);
        let child = if stage == "recipe" {
            child.env("RUSTC", directory.join("missing-setup-rustc"))
        } else {
            child
        };
        let output = child.output().unwrap();
        assert!(!output.status.success());
        let marker = if stage == "recipe" {
            "missing-setup-rustc"
        } else {
            "invalid_manifest_marker"
        };
        let transcript = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
            .collect::<String>();
        assert!(transcript.contains(marker), "{stage}: {transcript}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        if !stderr.contains(marker) {
            failures.push(format!("{stage}: Cargo error absent from stderr: {stderr}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn browser_clean_preserves_authored_inputs() {
    let browser = Browser::new();
    let library = browser.path("target/maestro-browser/library");
    let example = browser.path("target/maestro-browser/example/retained");
    fs::create_dir_all(&library).unwrap();
    fs::create_dir_all(example.parent().unwrap()).unwrap();
    fs::write(library.join("generated"), "generated").unwrap();
    fs::write(&example, "retained").unwrap();
    let preparation = browser.path("target/maestro-browser/.author/library/inputs/retained");
    fs::create_dir_all(preparation.parent().unwrap()).unwrap();
    fs::write(&preparation, "prepared inputs").unwrap();
    let inputs = [
        preparation,
        browser.fixture("src/main.rs"),
        browser.fixture("Cargo.lock"),
        browser.path("assets/brand/brand.json"),
        browser.path("assets/brand/fonts/Barlow-Regular.ttf"),
        example,
    ];
    let before: Vec<_> = inputs.iter().map(|path| fs::read(path).unwrap()).collect();
    for _ in 0..2 {
        success(&browser.run("browser-clean"));
    }
    assert!(!library.exists());
    for (path, bytes) in inputs.iter().zip(before) {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[path = "fixtures/browser_support.rs"]
mod browser_support;
use browser_support::{Browser, success};
use egui::{self, Color32};
use std::fs;
#[path = "fixtures/browser_author/src/editor.rs"]
mod editor;

#[test]
fn browser_editor_focus_selection_caret_and_composition_use_identity() {
    let browser = Browser::new();
    for pack in ["assets/brand/brand.json", "assets/brand/packs/signal.json"] {
        let data: serde_json::Value =
            serde_json::from_slice(&fs::read(browser.path(pack)).unwrap()).unwrap();
        for mode in ["dark", "light"] {
            let colors = [
                "text",
                "background",
                "panel",
                "border",
                "accent",
                "on-accent",
            ]
            .map(|role| {
                let key = data["modes"][mode]["colors"][role].as_str().unwrap();
                Color32::from_hex(data["palette"][key].as_str().unwrap()).unwrap()
            });
            let visuals = editor::visuals(colors);
            assert_eq!(visuals.selection.stroke.color, colors[5]);
            let context = egui::Context::default();
            let mut fonts = fixture_fonts(&browser);
            fonts
                .families
                .insert(egui::FontFamily::Proportional, vec!["Barlow".into()]);
            context.set_fonts(fonts);
            context.set_visuals(visuals.clone());
            let mut text = "selected author text".to_owned();
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                let id = ui.make_persistent_id("author-input");
                ui.memory_mut(|memory| memory.request_focus(id));
                assert!(editor::show(ui, &mut text).has_focus());
            });
            output.textures_delta.clear();
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.stroke.color == colors[4] && rect.stroke.width.to_bits() == 2.0_f32.to_bits()
            )), "{pack} {mode}: focus frame must use accent, not selected-text foreground");
            assert_eq!(visuals.text_cursor.stroke.color, colors[0]);
            assert_eq!(
                visuals.ime_composition.active_underline_stroke.color,
                colors[0]
            );
            assert_eq!(
                visuals.ime_composition.inactive_underline_stroke.color,
                colors[0]
            );
        }
    }
}

#[test]
fn browser_editor_preserves_padding_and_paints_decoration_outside() {
    let context = egui::Context::default();
    let mut text = "author text".to_owned();
    let mut field = egui::Rect::NOTHING;
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let id = ui.make_persistent_id("author-input");
        ui.memory_mut(|memory| memory.request_focus(id));
        field = editor::show(ui, &mut text).rect;
    });
    output.textures_delta.clear();
    let text_position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.pos),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        text_position,
        field.min + egui::vec2(4.0, 2.0),
        "text keeps the field's default padding"
    );
    let decoration = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.stroke.width.to_bits() == 2.0_f32.to_bits() => {
                Some(rect)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(decoration.rect, field);
    assert_eq!(decoration.stroke_kind, egui::StrokeKind::Outside);
}

#[test]
fn browser_build_emits_wasm_and_prepared_assets() {
    let browser = Browser::new();
    let mut command = browser.command("browser-library-build");
    let log = browser_support::native_tool(&browser, &mut command, "wasm-opt");
    success(&command.output().unwrap());
    let optimizer = fs::read_to_string(log).unwrap();
    assert!(
        optimizer
            .lines()
            .any(|line| line.contains("-Oz") && line.contains("-o"))
    );
    let output = browser.path("target/maestro-browser/library");
    let files: Vec<_> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let wasm = files
        .iter()
        .find(|file| file.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    let bytes = fs::read(wasm).unwrap();
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[..8], b"\0asm\x01\0\0\0");
    for suffix in ["js", "ts", "html", "css"] {
        assert!(
            files
                .iter()
                .any(|file| file.extension().is_some_and(|ext| ext == suffix)),
            "{suffix}"
        );
    }
    let html = fs::read_to_string(output.join("index.html")).unwrap();
    for file in &files {
        if file
            .extension()
            .is_some_and(|ext| ext == "js" || ext == "wasm" || ext == "css")
        {
            assert!(html.contains(file.file_name().unwrap().to_str().unwrap()));
        }
    }
    let css = fs::read_to_string(output.join("app.css")).unwrap();
    for part in css.split("url(\"").skip(1) {
        assert!(output.join(part.split('"').next().unwrap()).is_file());
    }
    println!(
        "optimized distribution raw/gzip={:?}; wasm raw/gzip={:?}; fonts raw/gzip={:?}",
        browser_support::measure(&output),
        browser_support::measure(wasm),
        browser_support::measure(&browser.path("assets/brand/fonts"))
    );
    for entry in fs::read_dir(browser.path("assets/brand/fonts")).unwrap() {
        let file = entry.unwrap().path();
        assert_eq!(
            fs::read(output.join("fonts").join(file.file_name().unwrap())).unwrap(),
            fs::read(file).unwrap()
        );
    }
}

#[test]
fn browser_development_runs_build_and_example() {
    let workspace = Workspace::new();
    success(&workspace.run("browser-dev"));
    assert!(workspace.log().contains("|browser-author|serve|"));
    assert!(!workspace.log().contains("|browser-author|watch|"));
    assert!(!workspace.log().contains("clear"));
}

#[test]
fn browser_compile_watch_starts_no_server() {
    let workspace = Workspace::new();
    success(&workspace.run("browser-dev-compile"));
    let log = workspace.log();
    assert!(log.contains("|browser-author|watch|"));
    assert!(!log.contains("|browser-author|serve|"));
    assert!(!log.contains("serve") && !log.contains("clear"));
}

#[test]
fn browser_check_repairs_formatting() {
    let browser = Browser::new();
    browser.workspace_sources();
    for path in [
        browser.fixture("src/main.rs"),
        browser.path("crates/maestro-web/src/lib.rs"),
    ] {
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("\n\n{original}")).unwrap();
        success(&browser.run("browser-check"));
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
}

#[test]
fn browser_checks_reject_invalid_sources() {
    let browser = Browser::new();
    browser.workspace_sources();
    success(&browser.run("browser-check"));
    for (path, invalid) in [
        (browser.fixture("src/main.rs"), "\nnot Rust"),
        (browser.path("crates/maestro-web/src/lib.rs"), "\nnot Rust"),
        (browser.fixture("src/main.rs"), "\nfn unused() {}\n"),
    ] {
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{original}{invalid}")).unwrap();
        assert!(!browser.run("browser-check").status.success());
        fs::write(path, original).unwrap();
    }
    assert!(!browser.path("target/maestro-browser/library").exists());
    assert!(!browser.path("target/maestro-browser/example").exists());
}

#[test]
fn browser_example_development_serves_rebuilt_external_assets() {
    use browser_support::{get, start_output, wait_for};
    let browser = Browser::new();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let (pack, pack_target, mark_target) = linked_identity(&browser);
    let mut command = browser.command("browser-example-dev");
    command.args([pack.to_str().unwrap(), "dark", "--port", &port.to_string()]);
    let (child, receive, thread) = start_output(command);
    wait_for(&receive, "127.0.0.1:");
    let output = browser.path("target/maestro-browser/example");
    let wasm = fs::read_dir(&output)
        .unwrap()
        .map(|file| file.unwrap().path())
        .find(|file| file.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    let original = fs::read(&wasm).unwrap();
    let source = browser.fixture("src/main.rs");
    let text = fs::read_to_string(&source).unwrap().replace(
        "ui.label(&self.0)",
        "ui.label(format!(\"edited echo: {}\", self.0))",
    );
    crate::recipe_support::record(format!("phase: authored edit {source:?}"));
    fs::write(source, text).unwrap();
    wait_for(&receive, "✅ success");
    let (_, changed) = get(port, wasm.file_name().unwrap().to_str().unwrap());
    assert_ne!(changed, original);
    let css = browser.fixture("assets/app.css");
    let original = fs::read_to_string(&css).unwrap();
    crate::recipe_support::record(format!("phase: authored edit {css:?}"));
    fs::write(css, format!("{original}\ncanvas{{outline-style:dotted}}\n")).unwrap();
    wait_for(&receive, "✅ success");
    let (_, changed) = get(port, "app.css");
    assert!(
        String::from_utf8(changed)
            .unwrap()
            .contains("outline-style:dotted")
    );
    edit_linked_targets(&pack_target.0, &mark_target.0, || {
        wait_for(&receive, "✅ success");
        get(port, wasm.file_name().unwrap().to_str().unwrap()).1
    });
    replace_identity_twice(&browser, &pack, || {
        wait_for(&receive, "✅ success");
        get(port, wasm.file_name().unwrap().to_str().unwrap()).1
    });
    drop(child);
    drop(receive);
    crate::recipe_support::record("phase: reader join");
    thread.join().unwrap();
    crate::recipe_support::record("phase: reader joined");
}

#[test]
fn browser_compile_watch_rebuilds_external_identity() {
    use browser_support::{start_output, wait_for};
    let browser = Browser::new();
    let (pack, pack_target, mark_target) = linked_identity(&browser);
    let mut command = browser.command("browser-dev-compile");
    command.args([pack.to_str().unwrap(), "light"]);
    let (child, receive, thread) = start_output(command);
    wait_for(&receive, "✅ success");
    let wasm = browser.path("target/maestro-browser/library/maestro-browser-author_bg.wasm");
    edit_linked_targets(&pack_target.0, &mark_target.0, || {
        wait_for(&receive, "✅ success");
        fs::read(&wasm).unwrap()
    });
    replace_identity_twice(&browser, &pack, || {
        wait_for(&receive, "✅ success");
        fs::read(&wasm).unwrap()
    });
    drop(child);
    drop(receive);
    crate::recipe_support::record("phase: reader join");
    thread.join().unwrap();
    crate::recipe_support::record("phase: reader joined");
}

/// Start with linked files whose targets are outside every authored watch root.
fn linked_identity(browser: &Browser) -> (std::path::PathBuf, Workspace, Workspace) {
    let pack = external_identity(browser);
    let pack_target = Workspace::new();
    let mark_target = Workspace::new();
    for (authored, target) in [
        (pack.clone(), pack_target.0.join("pack.json")),
        (
            browser.path("external mark.svg"),
            mark_target.0.join("mark.svg"),
        ),
    ] {
        crate::recipe_support::record(format!("phase: authored edit {authored:?}"));
        fs::rename(&authored, &target).unwrap();
        std::os::unix::fs::symlink(target, authored).unwrap();
    }
    (pack, pack_target, mark_target)
}

/// Observe edits of both startup dependencies through their resolved targets.
fn edit_linked_targets(
    pack_directory: &std::path::Path,
    mark_directory: &std::path::Path,
    mut rebuilt: impl FnMut() -> Vec<u8>,
) {
    let pack = pack_directory.join("pack.json");
    let mut data: serde_json::Value = serde_json::from_slice(&fs::read(&pack).unwrap()).unwrap();
    data["wordmark"] = "linked target wordmark".into();
    crate::recipe_support::record(format!("phase: authored edit {pack:?}"));
    fs::write(pack, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    eprintln!("waiting for linked pack target rebuild");
    assert!(String::from_utf8_lossy(&rebuilt()).contains("linked target wordmark"));
    let mark = mark_directory.join("mark.svg");
    let svg = fs::read_to_string(&mark)
        .unwrap()
        .replace("<svg", "<svg data-revision=\"linked\"");
    crate::recipe_support::record(format!("phase: authored edit {mark:?}"));
    fs::write(mark, svg).unwrap();
    eprintln!("waiting for linked mark target rebuild");
    assert!(String::from_utf8_lossy(&rebuilt()).contains("data-revision=\"linked\""));
}

/// Replace directory entries as an editor does, including a newly selected dependency.
fn replace_identity_twice(
    browser: &Browser,
    pack: &std::path::Path,
    mut rebuilt: impl FnMut() -> Vec<u8>,
) {
    let mark = browser.path("external mark.svg");
    for revision in ["first", "second"] {
        let mut data: serde_json::Value = serde_json::from_slice(&fs::read(pack).unwrap()).unwrap();
        data["wordmark"] = format!("{revision} author wordmark").into();
        let temporary = pack.with_extension("tmp");
        crate::recipe_support::record(format!("phase: authored edit {temporary:?}"));
        fs::write(&temporary, serde_json::to_string_pretty(&data).unwrap()).unwrap();
        crate::recipe_support::record(format!("phase: authored edit {temporary:?}"));
        fs::rename(temporary, pack).unwrap();
        eprintln!("waiting for {revision} pack replacement rebuild");
        assert!(
            String::from_utf8_lossy(&rebuilt()).contains(&format!("{revision} author wordmark"))
        );
        let temporary = mark.with_extension("tmp");
        let svg = fs::read_to_string(browser.path("assets/brand/mark.svg"))
            .unwrap()
            .replace("<svg", &format!("<svg data-revision=\"{revision}\""));
        crate::recipe_support::record(format!("phase: authored edit {temporary:?}"));
        fs::write(&temporary, svg).unwrap();
        crate::recipe_support::record(format!("phase: authored edit {temporary:?}"));
        fs::rename(temporary, &mark).unwrap();
        eprintln!("waiting for {revision} mark replacement rebuild");
        assert!(
            String::from_utf8_lossy(&rebuilt()).contains(&format!("data-revision=\"{revision}\""))
        );
    }
    let replacement = browser.path("new dependency/mark.svg");
    fs::create_dir_all(replacement.parent().unwrap()).unwrap();
    fs::copy(&mark, &replacement).unwrap();
    let mut data: serde_json::Value = serde_json::from_slice(&fs::read(pack).unwrap()).unwrap();
    data["mark"]["template"] = "../new dependency/mark.svg".into();
    crate::recipe_support::record(format!("phase: authored edit {pack:?}"));
    fs::write(pack, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    rebuilt();
    let contents = fs::read_to_string(&replacement)
        .unwrap()
        .replace("second", "replacement");
    crate::recipe_support::record(format!("phase: authored edit {replacement:?}"));
    fs::write(replacement, contents).unwrap();
    assert!(String::from_utf8_lossy(&rebuilt()).contains("data-revision=\"replacement\""));
}

/// Keep the selected pack and mark outside both existing watch roots.
fn external_identity(browser: &Browser) -> std::path::PathBuf {
    let pack = browser.path("external identity/pack.json");
    fs::create_dir_all(pack.parent().unwrap()).unwrap();
    let mark = browser.path("external mark.svg");
    fs::copy(browser.path("assets/brand/mark.svg"), &mark).unwrap();
    let mut data: serde_json::Value =
        serde_json::from_slice(&fs::read(browser.path("assets/brand/brand.json")).unwrap())
            .unwrap();
    data["mark"]["template"] = "../external mark.svg".into();
    crate::recipe_support::record(format!("phase: authored edit {pack:?}"));
    fs::write(&pack, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    pack
}

#[test]
fn browser_example_build_uses_shared_inputs() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    success(&browser.run("browser-example-build"));
    let output = browser.path("target/maestro-browser/example");
    assert!(output.join("index.html").is_file());
    assert!(fs::read_dir(&output).unwrap().any(|file| {
        file.unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "wasm")
    }));
    assert!(!browser.path("target/maestro-browser/library").exists());
    let workspace = Workspace::new();
    success(&workspace.run("browser-library-build"));
    success(&workspace.run("browser-example-build"));
    let log = workspace.log();
    let commands: Vec<_> = log
        .lines()
        .map(|line| line.split("|--dist|").next().unwrap())
        .collect();
    assert_eq!(commands.len(), 2);
    assert_eq!(
        commands[0],
        commands[1].replace("|example-build|", "|library-build|")
    );
}

#[test]
fn browser_preview_serves_without_author_tools() {
    use browser_support::{get, start_output, wait_for};
    let browser = Browser::new();
    let dist = browser.path("target/maestro-browser/example");
    fs::create_dir_all(&dist).unwrap();
    let files: [(&str, &[u8], &str); 6] = [
        (
            "index.html",
            b"<!doctype html><title>prepared</title>",
            "text/html",
        ),
        ("app.css", b"body{margin:0}", "text/css"),
        ("app.js", b"export default 1", "text/javascript"),
        ("app.wasm", b"\0asm\x01\0\0\0", "application/wasm"),
        ("font.ttf", b"prepared font bytes", "font/ttf"),
        ("OFL.txt", b"prepared notice", "text/plain"),
    ];
    for (name, bytes, _) in files {
        fs::write(dist.join(name), bytes).unwrap();
    }
    preview_tools(&browser);
    let mut command = browser.command("browser-example-preview");
    command
        .env("PATH", browser.path("bin"))
        .args(["--port", "0"]);
    let (child, receive, thread) = start_output(command);
    let startup = wait_for(&receive, "http://127.0.0.1:");
    let port: u16 = startup
        .rsplit("http://127.0.0.1:")
        .next()
        .unwrap()
        .trim()
        .trim_end_matches('/')
        .parse()
        .unwrap();
    for (name, bytes, mime) in files {
        let (headers, body) = get(port, name);
        assert!(headers.contains("200 OK"));
        assert!(headers.to_lowercase().contains(mime));
        assert_eq!(body, bytes);
        assert_eq!(fs::read(dist.join(name)).unwrap(), bytes);
    }
    assert_eq!(get(port, "nested/route").1, files[0].1);
    drop(child);
    drop(receive);
    crate::recipe_support::record("phase: reader join");
    thread.join().unwrap();
    crate::recipe_support::record("phase: reader joined");
    preview_failures(&browser);
}

#[test]
fn browser_example_check_emits_no_distribution() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    success(&browser.run("browser-example-check"));
    assert!(!browser.path("target/maestro-browser/library").exists());
    assert!(!browser.path("target/maestro-browser/example").exists());
    fs::write(browser.fixture("src/main.rs"), "fn main() { missing(); }").unwrap();
    assert!(!browser.run("browser-example-check").status.success());
    assert!(!browser.path("target/maestro-browser/library").exists());
    assert!(!browser.path("target/maestro-browser/example").exists());
}

#[test]
fn browser_example_clean_preserves_shared_assets() {
    let browser = Browser::new();
    for name in ["library", "example"] {
        fs::create_dir_all(browser.path(&format!("target/maestro-browser/{name}"))).unwrap();
        fs::write(
            browser.path(&format!("target/maestro-browser/{name}/retained")),
            "prepared",
        )
        .unwrap();
    }
    let preparation = browser.path("target/maestro-browser/.author/example/inputs/retained");
    fs::create_dir_all(preparation.parent().unwrap()).unwrap();
    fs::write(&preparation, "prepared inputs").unwrap();
    for _ in 0..2 {
        success(&browser.run("browser-example-clean"));
    }
    assert_eq!(fs::read_to_string(preparation).unwrap(), "prepared inputs");
    assert!(!browser.path("target/maestro-browser/example").exists());
    assert!(
        browser
            .path("target/maestro-browser/library/retained")
            .is_file()
    );
    assert!(browser.fixture("src/main.rs").is_file());
    assert!(
        browser
            .path("assets/brand/fonts/Barlow-Regular.ttf")
            .is_file()
    );
}

#[test]
fn browser_styles_reach_prepared_output() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let path = browser.fixture("assets/app.css");
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("margin:0", "margin:3px"),
    )
    .unwrap();
    success(&browser.run("browser-library-build"));
    let output =
        fs::read_to_string(browser.path("target/maestro-browser/library/app.css")).unwrap();
    assert!(output.contains("margin:3px"));
    assert!(!output.contains("margin:0"));
    assert!(output.contains("var(--maestro-color-text)"));
}

#[test]
fn browser_build_rejects_missing_or_failing_inputs() {
    let browser = Browser::new();
    let dist = browser.path("target/maestro-browser/library");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("sentinel"), "earlier distribution").unwrap();
    for path in [
        browser.fixture("index.html"),
        browser.path("assets/brand/fonts/Barlow-Regular.ttf"),
        browser.fixture("src/main.rs"),
    ] {
        let original = fs::read(&path).unwrap();
        if path.ends_with("main.rs") {
            fs::write(&path, "fn main() { missing(); }").unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let output = browser.run("browser-library-build");
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty() || !output.stdout.is_empty());
        fs::write(&path, original).unwrap();
        success(&browser.run("browser-library-build"));
    }
    let mut command = browser.command("browser-library-build");
    browser_support::native_tool(&browser, &mut command, "wasm-opt");
    command.env("MAESTRO_NATIVE_FAIL", "1");
    assert!(!command.output().unwrap().status.success());
    command.env_remove("MAESTRO_NATIVE_FAIL");
    success(&command.output().unwrap());
    let justfile = browser.path("justfile");
    let text = fs::read_to_string(&justfile).unwrap();
    fs::write(
        justfile,
        text.replace("browser-library-build pack=", "absent-library-build pack="),
    )
    .unwrap();
    assert!(!browser.run("browser-library-build").status.success());
}

#[test]
fn browser_wasm_closure_excludes_native_engines() {
    let browser = Browser::new();
    let manifest = browser.fixture("Cargo.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    let lock = fs::read(browser.fixture("Cargo.lock")).unwrap();
    let native = browser.fixture("native-only/src");
    fs::create_dir_all(&native).unwrap();
    fs::write(
        native.parent().unwrap().join("Cargo.toml"),
        "[package]\nname=\"native-only\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    fs::write(native.join("lib.rs"), "pub fn mode(p: std::fs::Permissions) -> u32 { use std::os::unix::fs::PermissionsExt; p.mode() }").unwrap();
    fs::write(
        &manifest,
        format!("{original}\n[dependencies.native-only]\npath=\"native-only\"\n"),
    )
    .unwrap();
    browser.generate_lockfile();
    let failed = browser.run("browser-library-build");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("could not find `unix`"));
    fs::write(&manifest, original).unwrap();
    fs::write(browser.fixture("Cargo.lock"), lock).unwrap();
    success(&browser.run("browser-library-build"));
    let tree = toolchain_support::command("cargo")
        .args([
            "tree",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "-e",
            "normal,features",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .unwrap();
    success(&tree);
    let tree = String::from_utf8(tree.stdout).unwrap();
    for absent in [
        "epaint_default_fonts",
        "egui-winit",
        "maestro-process",
        "wasmtime",
        "native-only",
    ] {
        assert!(!tree.contains(absent), "{absent}");
    }
    assert!(tree.contains("eframe feature \"glow\""));
}

#[test]
fn browser_debug_build_keeps_source_locations() {
    let browser = Browser::new();
    let mut command = browser.command("browser-debug-build");
    let log = browser_support::native_tool(&browser, &mut command, "wasm-opt");
    success(&command.output().unwrap());
    assert!(!log.exists());
    let output = browser.path("target/maestro-browser/debug");
    let wasm = fs::read_dir(&output)
        .unwrap()
        .map(|file| file.unwrap().path())
        .find(|file| file.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap();
    println!(
        "debug distribution raw/gzip={:?}; wasm raw/gzip={:?}",
        browser_support::measure(&output),
        browser_support::measure(&wasm)
    );
    let bytes = fs::read(wasm).unwrap();
    for needle in [b".debug_line".as_slice(), b"main.rs"] {
        assert!(bytes.windows(needle.len()).any(|part| part == needle));
    }
    assert!(fs::read_dir(output).unwrap().any(|file| {
        file.unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "ts")
    }));
}

#[test]
fn browser_assets_follow_relative_selected_identity() {
    let browser = Browser::new();
    for (pack, body) in [
        ("assets/brand/brand.json", "Barlow"),
        ("assets/brand/packs/signal.json", "Space Grotesk"),
    ] {
        for mode in ["dark", "light"] {
            success(
                &browser
                    .command("browser-library-build")
                    .arg(pack)
                    .arg(mode)
                    .output()
                    .unwrap(),
            );
            assert_identity_wasm(&browser, "library", pack, mode, body);
        }
    }
    for path in [
        browser.path("assets/brand/brand.json"),
        browser.path("assets/brand/mark.svg"),
    ] {
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(!browser.run("browser-library-build").status.success());
        fs::write(path, original).unwrap();
        success(&browser.run("browser-library-build"));
    }
    let pack = browser.path("assets/brand/brand.json");
    fs::write(pack, "{ malformed pack").unwrap();
    assert!(!browser.run("browser-library-build").status.success());
}

/// Observe every selected presentation role in the final consuming wasm artifact.
fn assert_identity_wasm(browser: &Browser, route: &str, pack: &str, mode: &str, body: &str) {
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(browser.path(pack)).unwrap()).unwrap();
    let bytes = browser_support::wasm(browser, route);
    let text = String::from_utf8_lossy(&bytes);
    for (role, key) in data["modes"][mode]["colors"].as_object().unwrap() {
        let color = data["palette"][key.as_str().unwrap()].as_str().unwrap();
        assert!(
            text.contains(&format!("--maestro-color-{role}:{color};")),
            "{pack} {mode} {role}"
        );
    }
    assert!(text.contains(&format!("--maestro-font-body:\"{body}\"")));
    assert!(text.contains("--maestro-type-body-size:16px"));
    assert!(text.contains("--maestro-font-mono:\"JetBrains Mono\""));
    assert!(text.contains("<svg"));
    assert!(!text.contains("{{wordmark}}") && !text.contains("var(--mark-star)"));
}

#[test]
fn browser_font_fallback_renders_required_symbols() {
    use egui::{FontFamily, FontId};
    let browser = Browser::new();
    let fonts = fixture_fonts(&browser);
    let context = egui::Context::default();
    context.set_fonts(fonts);
    let mut frame = context.run_ui(egui::RawInput::default(), |ui| {
        let star = layout_sign(ui, "✦", "alone");
        let missing = layout_sign(ui, "◻", "alone");
        assert_eq!(
            star.rows[0].glyphs[0].uv_rect,
            missing.rows[0].glyphs[0].uv_rect
        );
        for name in [
            "Barlow",
            "Condensed 600",
            "Condensed 700",
            "Mono",
            "Space Grotesk",
        ] {
            check_signs(ui, name);
            ui.label(
                egui::RichText::new("✦✓✗┃↑↓")
                    .font(FontId::new(16.0, FontFamily::Name(name.into()))),
            );
        }
    });
    assert!(
        !context
            .tessellate(std::mem::take(&mut frame.shapes), frame.pixels_per_point)
            .is_empty()
    );
    frame.textures_delta.clear();
}

#[test]
fn browser_asset_notices_match_distributed_files() {
    use std::process::Command;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, notice, digest) in [
        (
            "Barlow-Regular.ttf",
            "barlow-OFL.txt",
            "95aa02c7c43096e0dd44d787ba6216864a67157e402adab59b35572e0c1577ea",
        ),
        (
            "BarlowCondensed-SemiBold.ttf",
            "barlowcondensed-OFL.txt",
            "7b619d14bc2327509a9ef32b0890f709626f7ecc9ff61191c2a4314c5499d2d9",
        ),
        (
            "BarlowCondensed-Bold.ttf",
            "barlowcondensed-OFL.txt",
            "e476562ec9c1e16cf16475895b511f08c804f438cc9a9f80a44ea50a0eeb5b65",
        ),
        (
            "JetBrainsMono-official-Regular.ttf",
            "jetbrainsmono-official-OFL.txt",
            "a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f",
        ),
        (
            "SpaceGrotesk.ttf",
            "spacegrotesk-OFL.txt",
            "acad6de1fc93436f5c0f1f4137751ef04f1aea3063e7036535970ffcfbd79f72",
        ),
        (
            "NotoSansSymbols2-Regular.ttf",
            "notosanssymbols2-OFL.txt",
            "7d5fb73b7ca67a6798101741f5d280a3d016a56a197afcd4199dbb57b4b82a21",
        ),
    ] {
        let output = Command::new("sha256sum")
            .arg(root.join("assets/brand/fonts").join(file))
            .output()
            .unwrap();
        success(&output);
        assert!(
            String::from_utf8_lossy(&output.stdout).starts_with(digest),
            "{file}"
        );
        let text = fs::read_to_string(root.join("assets/brand/fonts").join(notice)).unwrap();
        assert!(text.contains("Copyright") && text.contains("SIL OPEN FONT LICENSE"));
    }
    let boost =
        fs::read_to_string(root.join("docs/licenses/author-tools/lockfree-object-pool.txt"))
            .unwrap();
    assert!(boost.starts_with("Boost Software License - Version 1.0"));
    for (tool, version) in [
        ("trunk", "0.21.14"),
        ("wasm-bindgen", "0.2.129"),
        ("wasm-opt", "133"),
        ("miniserve", "0.35.0"),
    ] {
        let output = Command::new(tool).arg("--version").output().unwrap();
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains(version));
    }
}

#[test]
fn browser_prepared_metadata_is_template_only() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let template = root.join(browser_support::FIXTURE).join("package.json.in");
    let actual: serde_json::Value = serde_json::from_slice(&fs::read(template).unwrap()).unwrap();
    assert_eq!(
        actual,
        serde_json::json!({
            "name": "@orchestration-maestro/maestro-web", "version": "@VERSION@", "type": "module",
            "main": "@MODULE_ENTRY@", "types": "@TYPE_ENTRY@",
            "exports": {".": "@MODULE_ENTRY@", "./app.css": "@STYLESHEET_ENTRY@"}
        })
    );
    assert!(
        !root
            .join("distribution/npm/maestro-web/package.json")
            .exists()
    );
    assert!(
        !root
            .join(browser_support::FIXTURE)
            .join("package.json")
            .exists()
    );
}

#[test]
fn browser_ignores_only_generated_outputs() {
    use std::process::Command;
    let browser = Browser::new();
    assert!(
        Command::new("git")
            .current_dir(&browser.workspace.0)
            .args(["init", "--quiet"])
            .status()
            .unwrap()
            .success()
    );
    for (path, ignored) in [
        ("target/debug/app.wasm", true),
        ("dist/index.html", true),
        ("dist-debug/index.html", true),
        (".trunk/cache", true),
        (".DS_Store", true),
        ("src/main.rs", false),
        ("Cargo.lock", false),
        ("Cargo.toml", false),
        ("index.html", false),
        ("debug.html", false),
        ("Trunk.json", false),
        ("assets/app.css", false),
        ("package.json.in", false),
    ] {
        let result = Command::new("git")
            .current_dir(&browser.workspace.0)
            .args(["check-ignore", "--no-index", "--quiet"])
            .arg(browser.fixture(path))
            .status()
            .unwrap();
        assert_eq!(result.success(), ignored, "{path}");
    }
    for path in [
        "assets/brand/brand.json",
        "assets/brand/fonts/Barlow-Regular.ttf",
        "assets/brand/mark.svg",
    ] {
        assert!(
            !Command::new("git")
                .current_dir(&browser.workspace.0)
                .args(["check-ignore", "--no-index", "--quiet"])
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn browser_recipes_preserve_literal_paths_and_failures() {
    let mut workspace = Workspace::new();
    let renamed = workspace.0.with_file_name(format!(
        "Maestro's author 東京 {}",
        workspace.0.file_name().unwrap().to_str().unwrap()
    ));
    fs::rename(&workspace.0, &renamed).unwrap();
    workspace.0 = renamed;
    check_command_failures(&workspace);
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(
        &workspace
            .command("browser-library-build")
            .args(["pack's 東京 path", "light", "--public-url", "/a space/"])
            .output()
            .unwrap(),
    );
    let log = workspace.log();
    assert!(
        log.contains("|browser-author|library-build|pack's 東京 path|light|--public-url|/a space/")
    );
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(&workspace.run("browser-build"));
    let log = workspace.log();
    assert_eq!(log.lines().count(), 2);
    assert!(
        log.lines()
            .next()
            .unwrap()
            .contains("|browser-author|library-build|")
    );
    assert!(
        log.lines()
            .nth(1)
            .unwrap()
            .contains("|browser-author|example-build|")
    );
    fs::remove_file(workspace.0.join("log")).unwrap();
    assert!(
        !workspace
            .command("browser-build")
            .env("MAESTRO_FAIL_ARG", "library-build")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(workspace.log().lines().count(), 1);
    fs::remove_file(workspace.0.join("log")).unwrap();
    check_root_browser_selection(&workspace);
    check_real_author_literals();
}

/// Exercise literal selected paths and native options through the actual bundler.
fn check_real_author_literals() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let pack = browser.path("assets/brand/pack's 東京.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(browser.path("assets/brand/brand.json")).unwrap())
            .unwrap();
    value["wordmark"] = "literal selected identity".into();
    fs::write(&pack, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    success(
        &browser
            .command("browser-library-build")
            .arg(&pack)
            .args(["light", "--public-url", "/a space/"])
            .output()
            .unwrap(),
    );
    assert!(
        browser_support::wasm(&browser, "library")
            .windows(b"literal selected identity".len())
            .any(|part| part == b"literal selected identity")
    );
    let html =
        fs::read_to_string(browser.path("target/maestro-browser/library/index.html")).unwrap();
    assert!(html.contains("/a space/"), "{html}");
}

/// Keep full and compile-only root selections on their distinct browser operations.
fn check_root_browser_selection(workspace: &Workspace) {
    success(&workspace.run("dev"));
    let log = workspace.log();
    assert!(
        log.contains("watch|build|--workspace|--locked") && log.contains("|browser-author|serve|")
    );
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(&workspace.run("dev-compile"));
    let log = workspace.log();
    assert!(
        log.contains("watch|build|-p|maestro-models|--locked")
            && log.contains("|browser-author|watch|")
    );
    assert!(!log.contains("|browser-author|serve|"));
}

/// Install only the preview executable, recipe runner and shell on its PATH.
fn preview_tools(browser: &Browser) {
    use recipe_support::install_executable;
    use std::process::Command;
    let binary = Command::new("which").arg("miniserve").output().unwrap();
    install_executable(
        std::path::Path::new(std::str::from_utf8(&binary.stdout).unwrap().trim()),
        &browser.path("bin/miniserve"),
    );
    for name in ["cargo", "trunk", "mise", "prek", "watchexec"] {
        fs::remove_file(browser.path("bin").join(name)).unwrap();
    }
    let just = Command::new("which").arg("just").output().unwrap();
    std::os::unix::fs::symlink(
        std::str::from_utf8(&just.stdout).unwrap().trim(),
        browser.path("bin/just"),
    )
    .unwrap();
    std::os::unix::fs::symlink("/bin/sh", browser.path("bin/sh")).unwrap();
}

/// Register whole physical faces without enabling the toolkit default bundle.
fn fixture_fonts(browser: &Browser) -> egui::FontDefinitions {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::empty();
    for (name, file) in [
        ("Barlow", "Barlow-Regular.ttf"),
        ("Condensed 600", "BarlowCondensed-SemiBold.ttf"),
        ("Condensed 700", "BarlowCondensed-Bold.ttf"),
        ("Mono", "JetBrainsMono-official-Regular.ttf"),
        ("Space Grotesk", "SpaceGrotesk.ttf"),
        ("Symbols", "NotoSansSymbols2-Regular.ttf"),
    ] {
        fonts.font_data.insert(
            name.into(),
            FontData::from_owned(fs::read(browser.path("assets/brand/fonts").join(file)).unwrap())
                .into(),
        );
    }
    for name in [
        "Barlow",
        "Condensed 600",
        "Condensed 700",
        "Mono",
        "Space Grotesk",
    ] {
        fonts.families.insert(
            FontFamily::Name(name.into()),
            vec![name.into(), "Mono".into(), "Symbols".into()],
        );
    }
    fonts
        .families
        .insert(FontFamily::Name("alone".into()), vec!["Barlow".into()]);
    fonts
}

/// Reject replacement boxes by observing actual glyph geometry and texture coordinates.
fn check_signs(ui: &egui::Ui, name: &str) {
    for sign in "✦✓✗┃↑↓".chars() {
        let actual = layout_sign(ui, &sign.to_string(), name);
        let glyph = &actual.rows[0].glyphs[0];
        assert!(glyph.advance_width > 0.0);
        for replacement in ["◻", "?"] {
            let other = layout_sign(ui, replacement, name);
            let other = &other.rows[0].glyphs[0];
            assert!(
                glyph.uv_rect != other.uv_rect
                    || glyph.advance_width.to_bits() != other.advance_width.to_bits(),
                "{name} {sign}"
            );
        }
    }
}

/// Lay out one sign through the selected real font chain.
fn layout_sign(ui: &egui::Ui, text: &str, family: &str) -> std::sync::Arc<egui::Galley> {
    ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            text.into(),
            egui::FontId::new(16.0, egui::FontFamily::Name(family.into())),
            egui::Color32::WHITE,
        )
    })
}

/// Exercise each independent public command failure through its own child boundary.
fn check_command_failures(workspace: &Workspace) {
    for (recipe, phase) in [
        ("browser-clean", "clean"),
        ("browser-library-build", "library-build"),
        ("browser-dev", "serve"),
        ("browser-dev-compile", "watch"),
        ("browser-check", "check"),
        ("browser-example-clean", "clean"),
        ("browser-example-build", "example-build"),
        ("browser-example-dev", "serve"),
        ("browser-example-check", "example-check"),
        ("browser-example-preview", "--index"),
    ] {
        let mut command = workspace.command(recipe);
        if recipe == "browser-example-preview" {
            command.env(
                "MAESTRO_FAIL",
                workspace.0.join("target/maestro-browser/example"),
            );
        } else if recipe.ends_with("clean") {
            command.env("MAESTRO_FAIL", phase);
        } else {
            command.env("MAESTRO_FAIL_ARG", phase);
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(17),
            "{recipe}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Native preview rejects a test-owned busy port and an absent distribution.
fn preview_failures(browser: &Browser) {
    let occupied = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    assert!(
        !browser
            .command("browser-example-preview")
            .args(["--port", &occupied.local_addr().unwrap().port().to_string()])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::remove_dir_all(browser.path("target/maestro-browser/example")).unwrap();
    assert!(
        !browser
            .command("browser-example-preview")
            .args(["--port", "0"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn browser_author_metadata_adapter_keeps_literals_and_status() {
    use std::{ffi::OsString, os::unix::fs::PermissionsExt};
    let workspace = Workspace::new();
    let script = workspace.0.join("cargo-probe");
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\"\nprintf 'native diagnostic' >&2\nexit 17\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    for args in [
        vec![
            "metadata",
            "--manifest-path",
            "path with ' and 東京/Cargo.toml",
        ],
        vec!["metadata", "--locked", "--format-version", "1"],
        vec!["build", "--release"],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_browser-cargo-locked"))
            .args(&args)
            .env("MAESTRO_BROWSER_CARGO", &script)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(17));
        let mut expected = args.join("\n") + "\n";
        if args[0] == "metadata" && !args.contains(&"--locked") {
            expected.push_str("--locked\n");
        }
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(output.stderr, b"native diagnostic");
    }
    let args: Vec<OsString> = ["browser-author", "cargo", "metadata"]
        .map(OsString::from)
        .to_vec();
    let before: Vec<_> = std::env::vars_os().collect();
    assert_eq!(
        maestro_tooling::run(&args, &script, &workspace.0).unwrap(),
        std::process::ExitCode::from(17)
    );
    assert_eq!(std::env::vars_os().collect::<Vec<_>>(), before);
}

#[test]
fn browser_author_preparation_is_route_local() {
    let browser = Browser::new();
    success(&browser.run("browser-example-check"));
    let config = browser.path("target/maestro-browser/.author/example/Trunk.json");
    assert_authored_target(&browser, &config, "index.html");
    assert!(!browser.path("target/maestro-browser/example").exists());
    assert!(
        browser
            .path("target/maestro-browser/.author/example/inputs/wordmark.txt")
            .is_file()
    );
    let marker = install_authored_hook(&browser);
    let mut retained = Vec::new();
    for (recipe, route, mode) in [
        ("browser-library-build", "library", "dark"),
        ("browser-example-build", "example", "light"),
        ("browser-debug-build", "debug", "dark"),
    ] {
        if marker.exists() {
            fs::remove_file(&marker).unwrap();
        }
        success(
            &browser
                .command(recipe)
                .args(["assets/brand/packs/signal.json", mode])
                .output()
                .unwrap(),
        );
        assert!(marker.is_file(), "the actual native authored hook must run");
        assert_eq!(
            fs::read(&marker).unwrap(),
            fs::read(browser.fixture("assets/app.css")).unwrap()
        );
        let directory = browser.path(&format!("target/maestro-browser/.author/{route}"));
        retained.push((
            directory.join("inputs/text.txt"),
            fs::read(directory.join("inputs/text.txt")).unwrap(),
        ));
        for (path, bytes) in &retained {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
        let target = if route == "debug" {
            "debug.html"
        } else {
            "index.html"
        };
        assert_authored_target(&browser, &directory.join("Trunk.json"), target);
        assert_identity_wasm(
            &browser,
            route,
            "assets/brand/packs/signal.json",
            mode,
            "Space Grotesk",
        );
    }
}

/// Compare the generated target with the actual authored file identity.
fn assert_authored_target(browser: &Browser, config: &std::path::Path, target: &str) {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    assert_eq!(
        std::path::Path::new(value["build"]["target"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        browser.fixture(target).canonicalize().unwrap()
    );
}

/// Install a native authored hook whose filesystem effect the real bundler observes.
fn install_authored_hook(browser: &Browser) -> std::path::PathBuf {
    let marker = browser.fixture("hook.css");
    let authored = browser.fixture("Trunk.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&fs::read(&authored).unwrap()).unwrap();
    configuration["hooks"] = serde_json::json!([{
        "stage": "pre_build",
        "command": "cp",
        "command_arguments": ["assets/app.css", "hook.css"]
    }]);
    fs::write(authored, serde_json::to_vec_pretty(&configuration).unwrap()).unwrap();
    marker
}

#[test]
fn browser_author_watch_ignores_generated_outputs_and_keeps_authored_roots() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let pack = external_identity(&browser);
    let extra = browser.path("authored-extra");
    fs::create_dir(&extra).unwrap();
    let ignored = browser.fixture("ignored.txt");
    crate::recipe_support::record(format!("phase: authored edit {ignored:?}"));
    fs::write(&ignored, "ignored input").unwrap();
    let authored = browser.fixture("Trunk.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&authored).unwrap()).unwrap();
    config["watch"]["watch"]
        .as_array_mut()
        .unwrap()
        .push("../../../../../authored-extra".into());
    config["watch"]["ignore"] = serde_json::json!(["ignored.txt"]);
    crate::recipe_support::record(format!("phase: authored edit {authored:?}"));
    fs::write(authored, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    assert_authored_watch_routes(&browser, &pack, &ignored, &extra);
}

/// Check both native watch routes with generated, ignored and authored input writes.
fn assert_authored_watch_routes(
    browser: &Browser,
    pack: &std::path::Path,
    ignored: &std::path::Path,
    extra: &std::path::Path,
) {
    use browser_support::{start_output, wait_for};
    for (recipe, route) in [
        ("browser-dev-compile", "library"),
        ("browser-example-dev", "example"),
    ] {
        let mut command = browser.command(recipe);
        command.args([
            pack.to_str().unwrap(),
            "dark",
            "--log",
            "info,trunk::watch=trace",
        ]);
        if route == "example" {
            command.args(["--port", "0"]);
        }
        let (child, receive, thread) = start_output(command);
        wait_for(&receive, "✅ success");
        for file in ["Trunk.json", "inputs/text.txt"] {
            let relative = format!("target/maestro-browser/.author/{route}/{file}");
            let path = browser.path(&relative);
            crate::recipe_support::record(format!("phase: authored edit {path:?}"));
            fs::write(&path, fs::read(&path).unwrap()).unwrap();
            assert_ignored_event(&receive, &relative);
        }
        crate::recipe_support::record(format!("phase: authored edit {ignored:?}"));
        fs::write(ignored, format!("ignored {route}")).unwrap();
        assert_ignored_event(&receive, "ignored.txt");
        crate::recipe_support::record(format!(
            "phase: authored edit {:?}",
            extra.join("source.txt")
        ));
        fs::write(extra.join("source.txt"), route).unwrap();
        wait_for(&receive, "accepted change in");
        wait_for(&receive, "✅ success");
        let css = browser.fixture("assets/app.css");
        let text =
            fs::read_to_string(&css).unwrap() + &format!("\ncanvas{{outline-{route}:dotted}}\n");
        crate::recipe_support::record(format!("phase: authored edit {css:?}"));
        fs::write(css, text).unwrap();
        wait_for(&receive, "accepted change in");
        wait_for(&receive, "✅ success");
        assert!(
            fs::read_to_string(browser.path(&format!("target/maestro-browser/{route}/app.css")))
                .unwrap()
                .contains(&format!("outline-{route}:dotted"))
        );
        drop(child);
        drop(receive);
        crate::recipe_support::record("phase: reader join");
        thread.join().unwrap();
        crate::recipe_support::record("phase: reader joined");
    }
}

/// Observe the native watcher's completed relevance decision for one actual write.
fn assert_ignored_event(receive: &std::sync::mpsc::Receiver<String>, path: &str) {
    crate::recipe_support::record(format!("phase: waiting for ignored-event decision {path}"));
    loop {
        let line = receive.recv().unwrap();
        if line.contains("change detected in")
            && line.contains(path)
            && (line.contains("Modify(Data(") || line.contains("Create("))
        {
            crate::recipe_support::record("phase: waiting for adjacent relevance decision");
            let decision = receive.recv().unwrap();
            assert!(
                decision.contains("Event not relevant, skipping"),
                "{line}\n{decision}"
            );
            break;
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn browser_author_native_hook_preserves_overrides_environment_and_failure() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    fs::create_dir(browser.fixture("scripts")).unwrap();
    let script = browser.path("hook-source");
    fs::write(&script, "#!/bin/sh\nset -eu\ncp assets/app.css hook.css\nprintf '%s\\n' \"$1\" \"$TRUNK_PROFILE\" \"$TRUNK_HTML_FILE\" \"$TRUNK_SOURCE_DIR\" \"$TRUNK_DIST_DIR\" \"$TRUNK_PUBLIC_URL\" \"$TRUNK_STAGING_DIR\" \"$MAESTRO_HOOK_INHERITED\" > hook-environment\nexit \"$MAESTRO_HOOK_STATUS\"\n").unwrap();
    recipe_support::install_executable(&script, &browser.fixture("scripts/hook.sh"));
    let authored = browser.fixture("Trunk.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&authored).unwrap()).unwrap();
    config["hooks"] = serde_json::json!([{
        "stage": "pre_build", "command": "must-not-run", "windows": null, "macos": null,
        "linux": {"command": "./scripts/hook.sh", "command_arguments": ["literal ' 東京"]}
    }]);
    fs::write(authored, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    for status in ["0", "17"] {
        let output = browser
            .command("browser-debug-build")
            .env("MAESTRO_HOOK_INHERITED", "inherited-value")
            .env("MAESTRO_HOOK_STATUS", status)
            .output()
            .unwrap();
        if status == "0" {
            success(&output);
        } else {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stdout).contains("returned a bad status"));
        }
        assert_eq!(
            fs::read(browser.fixture("hook.css")).unwrap(),
            fs::read(browser.fixture("assets/app.css")).unwrap()
        );
        assert_hook_environment(&browser);
    }
}

/// Compare the environment received by the native authored executable.
#[cfg(target_os = "linux")]
fn assert_hook_environment(browser: &Browser) {
    let environment = fs::read_to_string(browser.fixture("hook-environment")).unwrap();
    let lines: Vec<_> = environment.lines().collect();
    assert_eq!(lines[0], "literal ' 東京");
    assert_eq!(lines[1], "debug");
    assert_eq!(
        std::path::Path::new(lines[2]),
        browser.fixture("debug.html").canonicalize().unwrap()
    );
    assert_eq!(
        std::path::Path::new(lines[3]),
        browser.fixture("").canonicalize().unwrap()
    );
    assert_eq!(
        std::path::Path::new(lines[4]),
        browser
            .path("target/maestro-browser/debug")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(lines[5], "/");
    assert_eq!(
        std::path::Path::new(lines[6]),
        browser
            .path("target/maestro-browser/debug")
            .canonicalize()
            .unwrap()
            .join(".stage")
    );
    assert_eq!(lines[7], "inherited-value");
}

#[test]
fn browser_author_hook_uses_resolved_configuration_directory() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let directory = browser.path("linked configuration");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("source.css"), "linked source").unwrap();
    let authored = browser.fixture("Trunk.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&authored).unwrap()).unwrap();
    config["hooks"] = serde_json::json!([{
        "stage": "pre_build", "command": "cp", "command_arguments": ["source.css", "hook.css"]
    }]);
    let target = directory.join("Trunk.json");
    fs::write(&target, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    fs::remove_file(&authored).unwrap();
    std::os::unix::fs::symlink(target, authored).unwrap();
    success(&browser.run("browser-debug-build"));
    assert_eq!(
        fs::read(directory.join("hook.css")).unwrap(),
        b"linked source"
    );
}

#[test]
fn browser_author_hook_status_propagates() {
    let workspace = Workspace::new();
    let args = [
        "browser-author",
        "hook",
        workspace.0.to_str().unwrap(),
        "sh",
        "-c",
        "exit 17",
    ]
    .map(std::ffi::OsString::from);
    assert_eq!(
        maestro_tooling::run(&args, std::path::Path::new(env!("CARGO")), &workspace.0).unwrap(),
        std::process::ExitCode::from(17)
    );
}

#[test]
fn browser_author_check_preserves_adjacent_locks() {
    let browser = Browser::new();
    let before = browser_support::authored(&browser);
    success(&browser.run("browser-example-check"));
    assert_eq!(browser_support::authored(&browser), before);
    for path in [browser.fixture("Cargo.lock"), browser.path("Cargo.lock")] {
        let original = fs::read(&path).unwrap();
        fs::write(&path, "version = 4\n").unwrap();
        let damaged = browser_support::authored(&browser);
        assert!(!browser.run("browser-example-check").status.success());
        assert_eq!(browser_support::authored(&browser), damaged);
        fs::write(path, original).unwrap();
        success(&browser.run("browser-example-check"));
    }
    success(&browser.run("browser-check"));
    assert_eq!(browser_support::authored(&browser), before);
    for route in ["library", "example", "debug"] {
        assert!(
            !browser
                .path(&format!("target/maestro-browser/{route}"))
                .exists()
        );
    }
}

#[test]
fn browser_author_trunk_rejects_stale_lock_without_writes() {
    use browser_support::start_output;
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let lock = browser.fixture("Cargo.lock");
    let original = fs::read(&lock).unwrap();
    for recipe in [
        "browser-library-build",
        "browser-example-build",
        "browser-debug-build",
        "browser-dev",
        "browser-dev-compile",
    ] {
        fs::write(&lock, "version = 4\n").unwrap();
        let before = browser_support::authored(&browser);
        if ["browser-dev", "browser-dev-compile"].contains(&recipe) {
            let mut command = browser.command(recipe);
            if recipe == "browser-dev" {
                command.args([
                    browser.path("assets/brand/brand.json").to_str().unwrap(),
                    "dark",
                    "--port",
                    "0",
                ]);
            }
            let (child, receive, thread) = start_output(command);
            let transcript = browser_support::rejected_pipeline(&receive);
            assert!(transcript.contains("--locked"), "{transcript}");
            drop(child);
            crate::recipe_support::record("phase: reader join");
            thread.join().unwrap();
            crate::recipe_support::record("phase: reader joined");
        } else {
            let failed = browser.run(recipe);
            assert!(!failed.status.success());
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&failed.stdout),
                String::from_utf8_lossy(&failed.stderr)
            );
            assert!(text.contains("--locked"), "{text}");
        }
        assert_eq!(browser_support::authored(&browser), before);
        for route in ["library", "example", "debug"] {
            assert!(
                !browser
                    .path(&format!("target/maestro-browser/{route}/index.html"))
                    .is_file()
            );
        }
        fs::write(&lock, &original).unwrap();
    }
    success(&browser.run("browser-library-build"));
}

#[test]
fn browser_author_included_files_rebuild_without_source_changes() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let source = fs::read(browser.fixture("src/main.rs")).unwrap();
    success(&browser.run("browser-library-build"));
    let before = browser_support::wasm(&browser, "library");
    success(
        &browser
            .command("browser-example-check")
            .args(["assets/brand/brand.json", "unknown-mode"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read(browser.path("target/maestro-browser/.author/library/inputs/brand.css")).unwrap(),
        fs::read(browser.path("target/maestro-browser/.author/example/inputs/brand.css")).unwrap()
    );
    let pack = browser.path("assets/brand/brand.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&pack).unwrap()).unwrap();
    value["wordmark"] = "included-input-marker".into();
    fs::write(pack, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    success(&browser.run("browser-library-build"));
    let bytes = browser_support::wasm(&browser, "library");
    assert!(
        bytes
            .windows(b"included-input-marker".len())
            .any(|part| part == b"included-input-marker")
    );
    assert!(
        !before
            .windows(b"included-input-marker".len())
            .any(|part| part == b"included-input-marker")
    );
    assert_eq!(fs::read(browser.fixture("src/main.rs")).unwrap(), source);
}

#[test]
fn browser_author_projection_failure_prevents_dependent_build() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    for path in [
        browser.path("assets/brand/brand.json"),
        browser.path("assets/brand/mark.svg"),
    ] {
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let failed = browser.run("browser-library-build");
        assert!(!failed.status.success());
        assert!(!String::from_utf8_lossy(&failed.stdout).contains("starting build"));
        assert!(!browser.path("target/maestro-browser/library").exists());
        fs::write(path, original).unwrap();
    }
    let pack = browser.path("assets/brand/brand.json");
    let original = fs::read(&pack).unwrap();
    fs::write(&pack, "not JSON").unwrap();
    assert!(!browser.run("browser-library-build").status.success());
    fs::write(pack, original).unwrap();
    let directory = browser.path("target/maestro-browser/.author/library/inputs");
    if directory.exists() {
        fs::remove_dir_all(&directory).unwrap();
    }
    fs::write(&directory, "occupied").unwrap();
    assert!(!browser.run("browser-library-build").status.success());
    fs::remove_file(directory).unwrap();
    success(&browser.run("browser-library-build"));
}

#[test]
fn browser_author_unrelated_workspace_bump_keeps_fixture_lock() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let manifest = fs::read(browser.fixture("Cargo.toml")).unwrap();
    let lock = fs::read(browser.fixture("Cargo.lock")).unwrap();
    let unifier = browser.path("crates/maestro-dependencies");
    let original = fs::canonicalize(&unifier).unwrap();
    fs::remove_file(&unifier).unwrap();
    assert!(
        std::process::Command::new("cp")
            .args(["-a"])
            .arg(original)
            .arg(&unifier)
            .status()
            .unwrap()
            .success()
    );
    let path = unifier.join("Cargo.toml");
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("base64 = { version = \"0.23.1\" }"));
    fs::write(
        path,
        text.replace("base64 = { version = \"0.23.1\" }\n", ""),
    )
    .unwrap();
    success(
        &toolchain_support::command(env!("CARGO"))
            .current_dir(&browser.workspace.0)
            .args(["metadata", "--offline", "--format-version", "1"])
            .output()
            .unwrap(),
    );
    success(
        &toolchain_support::command(env!("CARGO"))
            .current_dir(&browser.workspace.0)
            .args(["metadata", "--locked", "--offline", "--format-version", "1"])
            .output()
            .unwrap(),
    );
    success(&browser.run("browser-example-check"));
    success(&browser.run("browser-library-build"));
    assert_eq!(fs::read(browser.fixture("Cargo.toml")).unwrap(), manifest);
    assert_eq!(fs::read(browser.fixture("Cargo.lock")).unwrap(), lock);
    assert_fixture_independent(&browser);
}

/// Qualify the actual resolved graph, not only the manifest spelling.
fn assert_fixture_independent(browser: &Browser) {
    let metadata = toolchain_support::command(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(browser.fixture("Cargo.toml"))
        .output()
        .unwrap();
    success(&metadata);
    let value: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let local: Vec<_> = value["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|package| package["source"].is_null())
        .collect();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0]["name"], "maestro-browser-author");
}

#[test]
fn browser_author_shared_launcher_uses_invoking_checkout() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = root
        .join("target/browser-author-tests")
        .canonicalize()
        .unwrap();
    success(
        &toolchain_support::command(env!("CARGO"))
            .current_dir(&root)
            .args(["build", "--locked", "-p", "maestro-tooling", "--bins"])
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap(),
    );
    let launcher = target.join("debug/development");
    let compiled = fs::read(&launcher).unwrap();
    let marker = install_authored_hook(&browser);
    success(&browser.run("browser-debug-build"));
    assert!(
        marker.is_file(),
        "Trunk must execute the invoking checkout's authored hook in its directory"
    );
    assert_eq!(
        fs::read(marker).unwrap(),
        fs::read(browser.fixture("assets/app.css")).unwrap()
    );
    assert_authored_target(
        &browser,
        &browser.path("target/maestro-browser/.author/debug/Trunk.json"),
        "debug.html",
    );
    assert_eq!(
        &browser_support::wasm(&browser, "debug")[..8],
        b"\0asm\x01\0\0\0"
    );
    assert_eq!(
        fs::read(launcher).unwrap(),
        compiled,
        "reuse the built launcher"
    );
}

#[test]
fn browser_author_bootstraps_from_empty_build_state() {
    let browser = Browser::new();
    browser_support::bootstrap_source(&browser);
    let (home, target) = browser_support::cold_sources(&browser);
    let before = browser_support::authored(&browser);
    let command = |recipe: &str| {
        let mut command = browser.command(recipe);
        command
            .env("CARGO_HOME", &home)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true");
        command
    };
    success(&command("browser-library-build").output().unwrap());
    let first = browser_support::wasm(&browser, "library");
    assert_eq!(&first[..8], b"\0asm\x01\0\0\0");
    let output = browser.path("target/maestro-browser/library");
    for extension in ["js", "ts", "css", "html"] {
        assert!(fs::read_dir(&output).unwrap().any(|file| {
            file.unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == extension)
        }));
    }
    assert_eq!(browser_support::authored(&browser), before);
    let pack = browser.path("assets/brand/brand.json");
    let original_pack = fs::read(&pack).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original_pack).unwrap();
    value["wordmark"] = "cold-included-rebuild".into();
    fs::write(&pack, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    success(&command("browser-library-build").output().unwrap());
    assert!(
        browser_support::wasm(&browser, "library")
            .windows(b"cold-included-rebuild".len())
            .any(|part| part == b"cold-included-rebuild")
    );
    fs::write(pack, original_pack).unwrap();
    let source = browser.fixture("src/main.rs");
    let original = fs::read(&source).unwrap();
    fs::write(&source, "fn main() { missing(); }").unwrap();
    assert!(
        !command("browser-library-build")
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(source, original).unwrap();
    success(&command("browser-library-build").output().unwrap());
    assert_eq!(browser_support::authored(&browser), before);
}
