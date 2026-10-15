#![cfg(test)]
#![cfg(unix)]

#[path = "fixtures/recipe_support.rs"]
mod recipe_support;
use recipe_support::Workspace;

#[test]
fn browser_clean_preserves_authored_inputs() {
    let browser = Browser::new();
    let library = browser.path("target/maestro-browser/library");
    let example = browser.path("target/maestro-browser/example/retained");
    fs::create_dir_all(&library).unwrap();
    fs::create_dir_all(example.parent().unwrap()).unwrap();
    fs::write(library.join("generated"), "generated").unwrap();
    fs::write(&example, "retained").unwrap();
    let inputs = [
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
use std::fs;

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
    assert!(workspace.log().starts_with("trunk serve|"));
    assert!(!workspace.log().contains("clear"));
}

#[test]
fn browser_compile_watch_starts_no_server() {
    let workspace = Workspace::new();
    success(&workspace.run("browser-dev-compile"));
    let log = workspace.log();
    assert!(log.starts_with("trunk watch|"));
    assert_eq!(log.lines().count(), 1);
    assert!(!log.contains("serve") && !log.contains("clear"));
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
    assert!(!browser.path("target/maestro-browser").exists());
}

#[test]
fn browser_example_development_serves_rebuilt_assets() {
    use browser_support::{get, start_output, wait_for};
    let browser = Browser::new();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut command = browser.command("browser-example-dev");
    command.args([
        browser.path("assets/brand/brand.json").to_str().unwrap(),
        "dark",
        "--port",
        &port.to_string(),
    ]);
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
    fs::write(source, text).unwrap();
    wait_for(&receive, "✅ success");
    let (_, changed) = get(port, wasm.file_name().unwrap().to_str().unwrap());
    assert_ne!(changed, original);
    let css = browser.fixture("assets/app.css");
    let original = fs::read_to_string(&css).unwrap();
    fs::write(css, format!("{original}\ncanvas{{outline-style:dotted}}\n")).unwrap();
    wait_for(&receive, "✅ success");
    let (_, changed) = get(port, "app.css");
    assert!(
        String::from_utf8(changed)
            .unwrap()
            .contains("outline-style:dotted")
    );
    let pack = browser.path("assets/brand/brand.json");
    let contents = fs::read_to_string(&pack).unwrap().replace(
        "\"wordmark\": \"Maestro\"",
        "\"wordmark\": \"Edited author wordmark\"",
    );
    fs::write(pack, contents).unwrap();
    wait_for(&receive, "✅ success");
    let (_, changed) = get(port, wasm.file_name().unwrap().to_str().unwrap());
    assert!(String::from_utf8_lossy(&changed).contains("Edited author wordmark"));
    drop(child);
    drop(receive);
    thread.join().unwrap();
}

#[test]
fn browser_example_build_uses_shared_inputs() {
    let browser = Browser::new();
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
    assert_eq!(commands[0], commands[1]);
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
    thread.join().unwrap();
    preview_failures(&browser);
}

#[test]
fn browser_example_check_emits_no_distribution() {
    let browser = Browser::new();
    success(&browser.run("browser-example-check"));
    assert!(!browser.path("target/maestro-browser").exists());
    fs::write(browser.fixture("src/main.rs"), "fn main() { missing(); }").unwrap();
    assert!(!browser.run("browser-example-check").status.success());
    assert!(!browser.path("target/maestro-browser").exists());
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
    for _ in 0..2 {
        success(&browser.run("browser-example-clean"));
    }
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
    use std::process::Command;
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
    assert!(
        Command::new("cargo")
            .args(["generate-lockfile", "--offline", "--manifest-path"])
            .arg(&manifest)
            .status()
            .unwrap()
            .success()
    );
    let failed = browser.run("browser-library-build");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("could not find `unix`"));
    fs::write(&manifest, original).unwrap();
    fs::write(browser.fixture("Cargo.lock"), lock).unwrap();
    success(&browser.run("browser-library-build"));
    let tree = Command::new("cargo")
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
fn browser_assets_follow_selected_identity() {
    let browser = Browser::new();
    for (pack, body) in [
        ("assets/brand/brand.json", "Barlow"),
        ("assets/brand/packs/signal.json", "Space Grotesk"),
    ] {
        let data: serde_json::Value =
            serde_json::from_slice(&fs::read(browser.path(pack)).unwrap()).unwrap();
        for mode in ["dark", "light"] {
            success(
                &browser
                    .command("browser-library-build")
                    .arg(browser.path(pack))
                    .arg(mode)
                    .output()
                    .unwrap(),
            );
            let wasm = fs::read_dir(browser.path("target/maestro-browser/library"))
                .unwrap()
                .map(|file| file.unwrap().path())
                .find(|file| file.extension().is_some_and(|ext| ext == "wasm"))
                .unwrap();
            let bytes = fs::read(wasm).unwrap();
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
        ("Trunk.toml", false),
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
            .env("MAESTRO_BRAND_RESULT", workspace.0.join("brand-result"))
            .args(["pack's 東京 path", "light", "--public-url", "/a space/"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read_to_string(workspace.0.join("brand-result")).unwrap(),
        "pack's 東京 path\nlight"
    );
    let log = workspace.log();
    assert!(log.contains("|--public-url|/a space/"));
    assert!(!log.contains("|pack's 東京 path|light"));
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(&workspace.run("browser-build"));
    let log = workspace.log();
    assert_eq!(log.lines().count(), 2);
    assert!(log.lines().next().unwrap().ends_with("/library"));
    assert!(log.lines().nth(1).unwrap().ends_with("/example"));
    fs::remove_file(workspace.0.join("log")).unwrap();
    assert!(
        !workspace
            .command("browser-build")
            .env("MAESTRO_FAIL", "build")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(workspace.log().lines().count(), 1);
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(&workspace.run("dev"));
    let log = workspace.log();
    assert!(log.contains("watch|build|--workspace|--locked") && log.contains("trunk serve|"));
    fs::remove_file(workspace.0.join("log")).unwrap();
    success(&workspace.run("dev-compile"));
    let log = workspace.log();
    assert!(log.contains("watch|build|-p|maestro-models|--locked") && log.contains("trunk watch|"));
    assert!(!log.contains("trunk serve|"));
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
        ("browser-library-build", "build"),
        ("browser-dev", "serve"),
        ("browser-dev-compile", "watch"),
        ("browser-check", "fmt"),
        ("browser-example-clean", "clean"),
        ("browser-example-build", "build"),
        ("browser-example-dev", "serve"),
        ("browser-example-check", "check"),
        ("browser-example-preview", "--index"),
    ] {
        let mut command = workspace.command(recipe);
        if recipe == "browser-example-preview" {
            command.env(
                "MAESTRO_FAIL",
                workspace.0.join("target/maestro-browser/example"),
            );
        } else {
            command.env("MAESTRO_FAIL", phase);
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
