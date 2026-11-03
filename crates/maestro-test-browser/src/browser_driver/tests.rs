//! Browser-independent inputs, routes and host-integrity observations.

use super::*;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The committed inventory used by the real browser qualification.
pub(super) const INVENTORY: &str =
    include_str!("../../../maestro-web/tests/sandbox_loading_support/exports.json");

/// Raw request targets that must never expose a packaged resource.
const DENIED_TARGETS: &[&str] = &[
    "/../../Cargo.toml",
    "/%2e%2e/Cargo.toml",
    "/..%2fsecret.js",
    "/%5c..%5cCargo.toml",
    "/helper.js%00",
    "/%",
    "/%FF",
    "/.",
    "/nested/",
];

/// Send a literal request target without a URL client's path normalization.
async fn raw_status(origin: &str, path: &str, status: u16) {
    let address = origin.strip_prefix("http://").unwrap();
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    socket.read_to_end(&mut response).await.unwrap();
    assert!(
        response.starts_with(format!("HTTP/1.1 {status} ").as_bytes()),
        "{path}: {}",
        String::from_utf8_lossy(&response)
    );
}

/// Run asynchronous unit observations on one controlled current-thread runtime.
pub(super) fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// A real listener returns CORS/CSP/MIME bytes and closes its endpoint on completion.
#[test]
fn server_serves_loopback_headers_and_closes_the_socket() {
    let scratch = Scratch::new().unwrap();
    fs::write(scratch.path().join("index.html"), "index").unwrap();
    fs::create_dir(scratch.path().join("nested")).unwrap();
    block_on(async {
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let server = Server::start(scratch.path()).await.unwrap();
        let origin = server.origin.clone();
        let response = client.get(format!("{origin}/")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.headers()["content-type"], "text/html");
        assert_eq!(response.text().await.unwrap(), "index");
        let response = client
            .get(format!("{origin}/contributions.html"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.headers()["content-security-policy"],
            resources::POLICY
        );
        for path in DENIED_TARGETS {
            raw_status(&origin, path, 403).await;
        }
        let missing = client
            .get(format!("{origin}/missing.js"))
            .send()
            .await
            .unwrap();
        assert_eq!(missing.status(), 404);
        assert_eq!(missing.headers()["access-control-allow-origin"], "*");
        assert!(missing.headers().get("content-type").is_none());
        assert!(missing.bytes().await.unwrap().is_empty());
        server.close().await.unwrap();
        assert!(client.get(origin).send().await.is_err());
    });
}

/// Encoded aliases receive the same contribution policy as the literal route.
#[test]
fn contribution_policy_follows_the_decoded_route() {
    let root = Scratch::new().unwrap();
    for path in ["/contributions.html", "/%63ontributions.html?cache=1"] {
        let response = resources::read(root.path(), path);
        assert_eq!(response.status, 200);
        assert_eq!(response.policy, Some(resources::POLICY));
    }
}

/// The executable version must match the supplied Chromium pin, not a partial prefix.
#[test]
fn chromium_version_rejects_missing_or_different_versions() {
    inputs::chromium_version("Chromium 156.0.8078.4\n").unwrap();
    for version in [
        "",
        "Chromium 155.0.8078.4",
        "Chromium 156.0.8078.40",
        "other",
    ] {
        assert!(inputs::chromium_version(version).is_err(), "{version}");
    }
    assert!(package::tools(Path::new("relative"), Path::new(".")).is_err());
}

/// Validate the actual inventory before any export may execute.
#[test]
fn inventory_rejects_empty_invalid_duplicate_and_extra_names() {
    let parsed = inputs::inventory(INVENTORY).unwrap();
    assert_eq!(
        parsed.families.each_ref().map(|(_, names)| names.len()),
        [26, 49, 194]
    );
    let value: Value = serde_json::from_str(INVENTORY).unwrap();
    for invalid in [
        Value::Null,
        json!({}),
        {
            let mut v = value.clone();
            v["execution"] = json!([]);
            v
        },
        {
            let mut v = value.clone();
            v["execution"][0] = json!("bad-name");
            v
        },
        {
            let mut v = value.clone();
            v["loading"][0] = v["execution"][0].clone();
            v
        },
        {
            let mut v = value.clone();
            v["extra"] = json!(true);
            v
        },
        {
            let mut v = value;
            v["expectedError"]["count"] = json!(2);
            v
        },
    ] {
        assert!(
            inputs::inventory(&invalid.to_string()).is_err(),
            "{invalid}"
        );
    }
}

/// Every authored browser export remains selected by the committed inventory.
#[test]
fn inventory_matches_authored_exports() {
    let inventory = inputs::inventory(INVENTORY).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-web/tests");
    for (family, selected) in &inventory.families[..2] {
        let source = fs::read_to_string(root.join(format!("sandbox_{family}_browser.rs"))).unwrap();
        let mut names = source
            .lines()
            .filter_map(|line| {
                line.strip_prefix("pub async fn ")
                    .and_then(|line| line.split_once("()"))
            })
            .map(|(name, _)| name.to_owned())
            .collect::<Vec<_>>();
        if *family == "loading" {
            for file in [
                "native_document_artifacts_browser.rs",
                "native_html_artifact_browser.rs",
                "sample_sessions_browser.rs",
                "native_remaining_artifacts_browser.rs",
            ] {
                names.extend(source_exports(&root.join(file)));
            }
            names.extend(
                source_exports(&root.join("native_attachment_browser.rs"))
                    .into_iter()
                    .filter(|name| name == "overlay_dom_documents_keep_resources_and_sheet_state"),
            );
        }
        names.sort();
        let mut selected = selected.clone();
        selected.sort();
        assert_eq!(selected, names, "{family}");
    }
    let mut names = Vec::new();
    for file in CONTRIBUTION_SOURCES {
        names.extend(
            source_exports(&root.join(file))
                .into_iter()
                .filter(|name| name != "overlay_dom_documents_keep_resources_and_sheet_state"),
        );
    }
    names.sort();
    let mut selected = inventory.families[2].1.clone();
    selected.sort();
    assert_eq!(selected, names);
}

/// Authored source modules supplying the contribution inventory.
const CONTRIBUTION_SOURCES: &[&str] = &[
    "dom_content_browser.rs",
    "contributions_browser.rs",
    "native_markdown_browser.rs",
    "native_thinking_browser.rs",
    "native_artifact_controls_browser.rs",
    "native_attachment_browser.rs",
    "native_messages_browser.rs",
    "native_tool_browser.rs",
    "native_simple_artifacts_browser.rs",
    "native_console_browser.rs",
    "custom_provider_card_browser.rs",
    "native_artifact_console_browser.rs",
    "document_tool_cards_browser.rs",
    "native_transcript_browser.rs",
    "message_tool_arguments.rs",
    "message_renderers_browser.rs",
    "message_errors.rs",
    "tool_renderers_browser/page.rs",
    "tool_renderer_support/lifetimes.rs",
    "dialogs_browser.rs",
    "provider_dialogs_browser.rs",
    "settings_dialogs_browser.rs",
    "message_editor_browser.rs",
    "agent_interface_browser.rs",
    "theme_toggle_browser.rs",
    "chat_panel_browser.rs",
    "chat_example_browser.rs",
];

/// Read only declarations exported with the authored browser-binding attribute.
fn source_exports(path: &Path) -> Vec<String> {
    let source = fs::read_to_string(path).unwrap();
    let lines = source.lines().collect::<Vec<_>>();
    lines
        .windows(2)
        .filter(|pair| {
            matches!(
                pair[0],
                "#[wasm_bindgen]" | "#[wasm_bindgen::prelude::wasm_bindgen]"
            )
        })
        .filter_map(|pair| pair[1].strip_prefix("pub "))
        .map(|line| line.strip_prefix("async ").unwrap_or(line))
        .filter_map(|line| {
            line.strip_prefix("fn ")
                .and_then(|line| line.split_once('('))
        })
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// Compiler diagnostics do not replace or duplicate the exact selected artifact.
#[test]
fn artifact_selects_exactly_one_emitted_wasm() {
    let message = json!({"reason":"compiler-artifact","target":{"name":"runtime","kind":["cdylib"]},"filenames":["/out/runtime.wasm","/out/runtime.d"]});
    let output = format!(
        "human diagnostic\n{}\n{message}\n",
        json!({"reason":"build-finished","success":true})
    );
    assert_eq!(
        inputs::artifact(&output, "cdylib", "runtime").unwrap(),
        "/out/runtime.wasm"
    );
    for invalid in [
        String::new(),
        format!("{message}\n{message}"),
        "{broken}".into(),
    ] {
        assert!(inputs::artifact(&invalid, "cdylib", "runtime").is_err());
    }
    assert!(inputs::artifact(&output, "test", "runtime").is_err());
    assert!(inputs::artifact(&output, "cdylib", "other").is_err());
}

/// Exercise MIME, query splitting, one decoding pass and fixture-relative helper aliases.
#[test]
fn server_preserves_packaged_resource_routes() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fs::create_dir(root.join("nested")).unwrap();
    for (file, bytes) in [
        ("index.html", "index"),
        ("helper.js", "helper"),
        ("module.wasm", "wasm"),
        ("雪 space.js", "unicode"),
        ("%2e%2e", "literal"),
        ("name#fragment.js", "fragment"),
        ("nested/item.js", "nested"),
        ("nested/helper.js", "decoy"),
    ] {
        fs::write(root.join(file), bytes).unwrap();
    }
    for (url, mime, bytes) in [
        ("/", "text/html", "index"),
        ("/?v=1", "text/html", "index"),
        ("/module.wasm", "application/wasm", "wasm"),
        ("/%E9%9B%AA%20space.js", "application/javascript", "unicode"),
        ("/%252e%252e", "application/octet-stream", "literal"),
        ("/name%23fragment.js", "application/javascript", "fragment"),
        ("/nested/./item.js", "application/javascript", "nested"),
        ("/nested//item.js", "application/javascript", "nested"),
        ("/nested/helper.js", "application/javascript", "helper"),
        (
            "/snippets/absent/helper.js",
            "application/javascript",
            "helper",
        ),
    ] {
        let response = resources::read(root, url);
        assert_eq!(response.status, 200, "{url}");
        assert_eq!(response.mime, Some(mime), "{url}");
        assert_eq!(response.bytes, bytes.as_bytes(), "{url}");
    }
}

/// The URL decoder retains question marks even when the native filesystem refuses that name.
#[test]
fn server_preserves_encoded_question_marks_with_the_native_filename_premise() {
    let root = Scratch::new().unwrap();
    let kept = root
        .keeps_distinct(&[std::ffi::OsStr::new("name?query.js")])
        .unwrap();
    assert_eq!(resource_path("/name%3Fquery.js").unwrap(), "/name?query.js");
    if kept {
        fs::write(root.path().join("name?query.js"), "question").unwrap();
    }
    let response = resources::read(root.path(), "/name%3Fquery.js");
    if kept {
        assert_eq!(response.status, 200);
        assert_eq!(response.bytes, b"question");
        assert_eq!(response.mime, Some("application/javascript"));
    } else {
        assert!(!response.status.is_success());
        assert!(response.bytes.is_empty());
        assert!(response.mime.is_none());
    }
}

/// Refused routes never return bytes; absence remains distinct from denial.
#[test]
fn server_refuses_traversal_malformed_paths_and_directories() {
    let scratch = Scratch::new().unwrap();
    fs::create_dir(scratch.path().join("nested")).unwrap();
    for path in DENIED_TARGETS {
        let response = resources::read(scratch.path(), path);
        assert_eq!(response.status, 403, "{path}");
        assert!(response.bytes.is_empty());
        assert!(response.mime.is_none());
    }
    assert_eq!(resources::read(scratch.path(), "/missing.js").status, 404);
}

/// Canonical containment admits internal links but denies external link chains.
#[cfg(unix)]
#[test]
fn server_preserves_canonical_root_and_internal_links_only() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().join("root");
    let outside = scratch.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(root.join("item.js"), "inside").unwrap();
    fs::write(outside.join("secret.js"), "outside").unwrap();
    symlink(root.join("item.js"), root.join("internal.js")).unwrap();
    symlink(outside.join("secret.js"), root.join("escape.js")).unwrap();
    symlink(&outside, root.join("escape-dir")).unwrap();
    symlink(root.join("escape.js"), root.join("chain.js")).unwrap();
    symlink(root.join("absent"), root.join("dangling.js")).unwrap();
    let alias = scratch.path().join("alias");
    symlink(&root, &alias).unwrap();
    for directory in [&root, &alias] {
        assert_eq!(resources::read(directory, "/internal.js").bytes, b"inside");
        for path in [
            "/escape.js".to_owned(),
            "/escape-dir/secret.js".into(),
            "/chain.js".into(),
            format!("/{}", outside.join("secret.js").display()),
            format!(
                "/%2f{}",
                outside
                    .join("secret.js")
                    .display()
                    .to_string()
                    .trim_start_matches('/')
            ),
            format!(
                "/../{}/helper.js",
                outside.file_name().unwrap().to_str().unwrap()
            ),
        ] {
            let response = resources::read(directory, &path);
            assert_eq!(response.status, 403, "{path}");
            assert!(response.bytes.is_empty(), "{path}");
            assert!(response.mime.is_none(), "{path}");
        }
        assert_eq!(resources::read(directory, "/dangling.js").status, 404);
    }
}

/// Package replacement is allowed, while every host byte and filename is covered.
#[test]
fn host_digest_excludes_packages_but_detects_host_changes() {
    let scratch = Scratch::new().unwrap();
    fs::create_dir(scratch.path().join("package")).unwrap();
    fs::write(scratch.path().join("host.js"), "host").unwrap();
    fs::write(scratch.path().join("package/module.js"), "one").unwrap();
    let before = resources::digest(scratch.path()).unwrap();
    fs::write(scratch.path().join("package/module.js"), "two").unwrap();
    assert_eq!(resources::digest(scratch.path()).unwrap(), before);
    fs::write(scratch.path().join("host.js"), "changed").unwrap();
    assert_ne!(resources::digest(scratch.path()).unwrap(), before);
    fs::write(scratch.path().join("host.js"), "host").unwrap();
    fs::rename(
        scratch.path().join("host.js"),
        scratch.path().join("other.js"),
    )
    .unwrap();
    assert_ne!(resources::digest(scratch.path()).unwrap(), before);
}

/// A missing executable is diagnosed without the pinned browser environment.
#[test]
fn launch_failures_are_reported() {
    let root = Scratch::new().unwrap();
    let executable = root.path().join("absent-chromium");
    let error = block_on(Browser::launch(
        &executable,
        &root.path().join("profile"),
        &[],
    ))
    .err()
    .expect("missing executable must fail");
    assert!(
        error
            .to_string()
            .contains(&executable.display().to_string()),
        "{error}"
    );
}
