//! Startup offline selections reaching the search-program manager.
#![cfg(test)]
#![cfg(unix)]
use super::read_support::{Fixture, Scratch, counting_fetch, isolated_os};
use super::runtime::run;
use maestro_tools::{SearchTool, ToolsManager, ToolsManagerOutput};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// The truth table both offline readers share: environment text, and whether it selects
/// offline.
const TRUTH: &[(Option<&str>, bool)] = &[
    (None, false),
    (Some(""), false),
    (Some("0"), false),
    (Some("false"), false),
    (Some("1"), true),
    (Some("true"), true),
    (Some("TRUE"), true),
    (Some("yes"), true),
    (Some("YeS"), true),
    (Some(" "), false),
    (Some("\u{feff}false"), false),
    (Some(" yes"), false),
    (Some("true "), false),
];

/// An explicit selection contradicting an ambient flag value, and whether the missing
/// program is still acquired.
const OVERRIDES: [(Option<bool>, &str, bool); 2] =
    [(Some(false), "1", true), (Some(true), "0", false)];

/// Places an executable program probe at `root/name` through a child process.
fn program(root: &Path, name: &str) {
    let source = root.join(format!("{name}.source"));
    std::fs::write(
        &source,
        "#!/bin/sh\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 99\nexit 0\n",
    )
    .unwrap();
    assert!(
        std::process::Command::new("install")
            .args(["-m", "0700"])
            .arg(&source)
            .arg(root.join(name))
            .status()
            .unwrap()
            .success()
    );
}

/// A manager over an empty binary directory and an empty `PATH`.
fn manager(offline: Option<bool>) -> (ToolsManager, Arc<AtomicUsize>, Scratch) {
    let calls = Arc::new(AtomicUsize::new(0));
    let bin = Scratch::new().unwrap();
    let manager = ToolsManager::new(bin.text(), Some(counting_fetch(&calls))).with_offline(offline);
    (manager, calls, bin)
}

/// Every entry name in the binary directory, so a staging directory cannot hide.
fn entries(bin: &Scratch) -> Vec<String> {
    std::fs::read_dir(bin.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn startup_selection_skips_missing_tool_downloads() {
    run(async {
        if !isolated_os(
            "startup_selection_skips_missing_tool_downloads",
            "offline",
            &[("PATH", Some(std::ffi::OsStr::new("")))],
        ) {
            return;
        }
        for (tool, name) in [(SearchTool::Fd, "fd"), (SearchTool::Rg, "ripgrep")] {
            let (manager, calls, bin) = manager(Some(true));
            let mut plain = Vec::new();
            let mut colored = Vec::new();
            assert_eq!(
                manager
                    .ensure_tool(
                        tool,
                        Some(ToolsManagerOutput {
                            stdout: &mut plain,
                            color: false
                        })
                    )
                    .await,
                None
            );
            assert_eq!(
                manager
                    .ensure_tool(
                        tool,
                        Some(ToolsManagerOutput {
                            stdout: &mut colored,
                            color: true
                        })
                    )
                    .await,
                None
            );
            assert_eq!(manager.ensure_tool(tool, None).await, None);
            let advice = format!("{name} not found. Offline mode enabled, skipping download.");
            assert_eq!(String::from_utf8(plain).unwrap(), format!("{advice}\n"));
            assert_eq!(
                String::from_utf8(colored).unwrap(),
                format!("\u{1b}[33m{advice}\u{1b}[39m\n")
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert!(entries(&bin).is_empty());
        }
    });
}

#[test]
fn startup_selection_keeps_installed_tool_discovery() {
    run(async {
        let bin = Scratch::new().unwrap();
        program(bin.path(), "fd");
        let calls = Arc::new(AtomicUsize::new(0));
        let manager =
            ToolsManager::new(bin.text(), Some(counting_fetch(&calls))).with_offline(Some(true));
        assert_eq!(
            manager.ensure_tool(SearchTool::Fd, None).await,
            Some(format!("{}/fd", bin.text())),
            "an offline selection still uses an installed program"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn startup_selection_overrides_contradictory_ambient_flags() {
    run(async {
        for (index, (selection, ambient, acquires)) in OVERRIDES.iter().enumerate() {
            let mode = index.to_string();
            if !isolated_os(
                "startup_selection_overrides_contradictory_ambient_flags",
                &mode,
                &[
                    ("PATH", Some(std::ffi::OsStr::new(""))),
                    ("MAESTRO_OFFLINE", Some(std::ffi::OsStr::new(ambient))),
                ],
            ) {
                continue;
            }
            let (manager, calls, bin) = manager(*selection);
            assert_eq!(manager.ensure_tool(SearchTool::Fd, None).await, None);
            assert_eq!(
                calls.load(Ordering::SeqCst),
                usize::from(*acquires),
                "selection {selection:?} over ambient {ambient}"
            );
            assert!(entries(&bin).is_empty());
        }
    });
}

#[test]
fn sdk_search_keeps_ambient_offline_policy() {
    run(async {
        for (index, (value, offline)) in TRUTH.iter().enumerate() {
            let mode = index.to_string();
            let offline_value = value.map(std::ffi::OsStr::new);
            if !isolated_os(
                "sdk_search_keeps_ambient_offline_policy",
                &mode,
                &[
                    ("PATH", Some(std::ffi::OsStr::new(""))),
                    ("MAESTRO_OFFLINE", offline_value),
                ],
            ) {
                continue;
            }
            let (manager, calls, _bin) = manager(None);
            assert_eq!(manager.ensure_tool(SearchTool::Fd, None).await, None);
            assert_eq!(
                calls.load(Ordering::SeqCst),
                usize::from(!offline),
                "case {value:?}"
            );
        }
    });
}
