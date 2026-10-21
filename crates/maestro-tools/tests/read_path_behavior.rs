//! Read-path transformations against the real filesystem and host lookups.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::read_support;
use crate::runtime::run;

use maestro_tools::{
    ReadToolOptions, create_read_tool_definition, expand_path, resolve_read_path, resolve_to_cwd,
};
use read_support::*;
#[cfg(unix)]
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn path_expansion_preserves_only_the_named_transforms() {
    const HOME: &str = "/explicit-home";
    let envs = [("HOME", Some(HOME)), ("USERPROFILE", Some(HOME))];
    if !isolated(
        "path_expansion_preserves_only_the_named_transforms",
        "home",
        &envs,
    ) {
        return;
    }
    let spaced: Vec<(String, String)> = [
        '\u{a0}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}',
        '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{202f}', '\u{205f}',
        '\u{3000}',
    ]
    .iter()
    .map(|space| (format!("a{space}b"), "a b".to_owned()))
    .collect();
    let kept = [
        "~other/file",
        "~\\notes",
        " a\t\n ",
        "a\u{85}b",
        "a\u{feff}b",
        "plain",
    ];
    let cases = [("@a", "a"), ("@@a", "@a"), ("@", "")]
        .map(|(from, to)| (from.to_owned(), to.to_owned()))
        .into_iter()
        .chain(spaced)
        .chain(kept.map(|path| (path.to_owned(), path.to_owned())));
    for (path, expected) in cases {
        assert_eq!(expand_path(&path).unwrap(), expected, "{path:?}");
    }
    assert_eq!(expand_path("~").unwrap(), HOME);
    assert_eq!(expand_path("~/notes").unwrap(), "/explicit-home/notes");
    assert_eq!(expand_path("@~/notes").unwrap(), "/explicit-home/notes");
}

#[test]
fn path_resolution_keeps_absolute_spelling_and_resolves_relative_inputs() {
    for (path, cwd, expected) in [
        ("/absolute/../kept", "/base", "/absolute/../kept"),
        ("x/../leaf", "/base", "/base/leaf"),
        ("", "/base", "/base"),
        ("@../leaf", "/base/inner", "/base/leaf"),
        ("/absolute", "relative", "/absolute"),
        ("./a//b/.", "/base", "/base/a/b"),
    ] {
        assert_eq!(resolve_to_cwd(path, cwd).unwrap(), expected, "{path:?}");
    }
    let process = std::env::current_dir().unwrap();
    assert_eq!(
        resolve_to_cwd("leaf", "relative").unwrap(),
        format!("{}/relative/leaf", process.to_str().unwrap())
    );
}

/// Files present, the spelling asked for, and the existing file that spelling must select.
type Fallback = (&'static [&'static str], &'static str, &'static str);

#[test]
fn read_paths_try_original_ampm_nfd_curly_then_combined() {
    const AM: &str = "shot 10.00\u{202f}AM.png";
    let cases: [Fallback; 14] = [
        (&[], "shot 10.00 AM.png", "shot 10.00 AM.png"),
        (&[AM], "shot 10.00 AM.png", AM),
        (
            &["cafe\u{301} 2 AM.", "caf\u{e9} 2\u{202f}AM."],
            "caf\u{e9} 2 AM.",
            "caf\u{e9} 2\u{202f}AM.",
        ),
        (
            &["shot 10.00\u{202f}Pm.png"],
            "shot 10.00 Pm.png",
            "shot 10.00\u{202f}Pm.png",
        ),
        (&["a AM.", "a\u{202f}AM."], "a AM.", "a AM."),
        (&["cafe\u{301}"], "caf\u{e9}", "cafe\u{301}"),
        (&["it's", "it\u{2019}s"], "it's", "it's"),
        (&["it\u{2019}s"], "it's", "it\u{2019}s"),
        (&["d'\u{e9}", "d\u{2019}\u{e9}"], "d'\u{e9}", "d'\u{e9}"),
        (&["n'e\u{301}", "n\u{2019}\u{e9}"], "n'\u{e9}", "n'e\u{301}"),
        (
            &["c\u{2019}\u{e9}\u{2019}", "c\u{2019}e\u{301}\u{2019}"],
            "c'\u{e9}\u{2019}",
            "c\u{2019}\u{e9}\u{2019}",
        ),
        (&["g\u{2019}e\u{301}"], "g'\u{e9}", "g\u{2019}e\u{301}"),
        (
            &["t 1\u{202f}AM. it\u{2019}s"],
            "t 1 AM. it's",
            "t 1 AM. it's",
        ),
        (&["am\u{202f}AM. x"], "am AM. x", "am\u{202f}AM. x"),
    ];
    for (index, (files, ask, found)) in cases.into_iter().enumerate() {
        let dir = TempDir::new();
        for file in files {
            dir.file(file, b"");
        }
        let resolved = resolve_read_path(ask, &dir.text()).unwrap();
        assert_eq!(resolved, format!("{}/{found}", dir.text()), "case {index}");
    }
    let dir = TempDir::new();
    std::fs::create_dir(dir.0.join("folder")).unwrap();
    assert_eq!(
        resolve_read_path("folder", &dir.text()).unwrap(),
        format!("{}/folder", dir.text())
    );
}

#[test]
fn read_paths_accept_lowercase_meridiem() {
    for suffix in ["am", "pm"] {
        let dir = TempDir::new();
        let chosen = format!("Screenshot 2024-01-01 at 10.00.00\u{202f}{suffix}.png");
        dir.file(&chosen, b"chosen");
        let ask = format!("Screenshot 2024-01-01 at 10.00.00 {suffix}.png");
        let resolved = resolve_read_path(&ask, &dir.text()).unwrap();
        assert_eq!(resolved, format!("{}/{chosen}", dir.text()));
        assert_eq!(std::fs::read(&resolved).unwrap(), b"chosen");
    }
}

#[cfg(unix)]
#[test]
fn read_paths_keep_lexical_symlink_identity() {
    let dir = TempDir::new();
    std::fs::create_dir_all(dir.0.join("a/b")).unwrap();
    dir.file("f", b"top");
    dir.file("a/f", b"under a");
    std::os::unix::fs::symlink(dir.0.join("a/b"), dir.0.join("link")).unwrap();
    let cwd = dir.text();
    assert_eq!(
        resolve_read_path("link", &cwd).unwrap(),
        format!("{cwd}/link")
    );
    assert_eq!(
        resolve_read_path("link/../f", &cwd).unwrap(),
        format!("{cwd}/f")
    );
    let tool = create_read_tool_definition::<()>(cwd, ReadToolOptions::default());
    let result = call(&tool, input("link/../f", None, None), None, None).unwrap();
    assert_eq!(only_text(&result), "top");
}

#[test]
fn native_path_host_lookups_are_lazy() {
    let test = "native_path_host_lookups_are_lazy";
    if isolated(test, "cwd", &[]) {
        let parent = TempDir::new();
        let gone = parent.0.join("gone");
        std::fs::create_dir(&gone).unwrap();
        std::env::set_current_dir(&gone).unwrap();
        std::fs::remove_dir(&gone).unwrap();
        assert!(std::env::current_dir().is_err());
        assert_eq!(
            resolve_to_cwd("/abs/file", "relative").unwrap(),
            "/abs/file"
        );
        assert_eq!(resolve_to_cwd("rel", "/base").unwrap(), "/base/rel");
        assert_eq!(expand_path("plain").unwrap(), "plain");
        assert!(resolve_to_cwd("rel", "relative-cwd").is_err());
    }
    if isolated(test, "home-explicit", &[("HOME", Some("/explicit"))]) {
        assert_eq!(expand_path("~").unwrap(), "/explicit");
        assert_eq!(expand_path("~/x").unwrap(), "/explicit/x");
    }
    if isolated(test, "home-absent", &[("HOME", None)]) {
        assert_eq!(expand_path("plain").unwrap(), "plain");
        let lookup = std::env::home_dir().map(|home| home.to_str().unwrap().to_owned());
        assert_eq!(
            expand_path("~/x").ok(),
            lookup.map(|home| format!("{home}/x"))
        );
    }
}

#[cfg(unix)]
#[test]
fn empty_home_is_used_verbatim() {
    if isolated(
        "empty_home_is_used_verbatim",
        "empty",
        &[("HOME", Some(""))],
    ) {
        assert_eq!(expand_path("plain").unwrap(), "plain");
        assert_eq!(expand_path("~/x").unwrap(), "/x");
        assert_eq!(expand_path("~").unwrap(), "");
    }
}

#[cfg(unix)]
#[test]
fn home_expansion_rejects_non_utf8_without_replacing_it() {
    let home = <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(b"/bytes-\xff-home");
    if isolated_os(
        "home_expansion_rejects_non_utf8_without_replacing_it",
        "bytes",
        &[("HOME", Some(home))],
    ) {
        let error = expand_path("~/x").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), "home directory is not valid UTF-8");
        assert_eq!(expand_path("plain").unwrap(), "plain");
    }
}

#[cfg(unix)]
#[test]
fn read_resolves_empty_and_absent_home_without_home_writes() {
    let test = "read_resolves_empty_and_absent_home_without_home_writes";
    let dir = TempDir::new();
    dir.file("MiXeD-Note.txt", b"empty home keeps the path root");
    let authored = format!("~{}/MiXeD-Note.txt", dir.text());
    if isolated(test, "empty", &[("HOME", Some(""))]) {
        let tool = create_read_tool_definition::<()>("/".into(), ReadToolOptions::default());
        let result = call(&tool, input(&authored, None, None), None, None).unwrap();
        assert_eq!(only_text(&result), "empty home keeps the path root");
    }
    if isolated(test, "absent", &[("HOME", None)]) {
        let home = std::env::home_dir().unwrap();
        let path = format!("{}{}/MiXeD-Note.txt", home.to_str().unwrap(), dir.text());
        let ops = Arc::new(ScriptedOps::new(b"account home"));
        let result = call(&tool_over(&ops), input(&authored, None, None), None, None).unwrap();
        assert_eq!(only_text(&result), "account home");
        assert_eq!(
            ops.log(),
            [
                format!("access:{path}"),
                format!("mime:{path}"),
                format!("read:{path}")
            ]
        );
    }
}

#[test]
fn read_paths_bind_before_future_poll() {
    if !isolated("read_paths_bind_before_future_poll", "bind", &[]) {
        return;
    }
    let (original, decoy) = (TempDir::new(), TempDir::new());
    original.file("f.txt", b"original");
    decoy.file("f.txt", b"decoy");
    std::env::set_current_dir(&original.0).unwrap();
    let tool = create_read_tool_definition::<()>(".".into(), ReadToolOptions::default());
    let pending = (tool.execute)("id".into(), input("f.txt", None, None), None, None, None);
    std::env::set_current_dir(&decoy.0).unwrap();
    assert_eq!(only_text(&run(pending).unwrap()), "original");
    let fresh = call(&tool, input("f.txt", None, None), None, None).unwrap();
    assert_eq!(only_text(&fresh), "decoy");
}

#[test]
fn scratch_directory_never_adopts_an_existing_candidate() {
    let root = TempDir::new();
    let tmp = root.0.join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let next = AtomicUsize::new(0);
    drop(TempDir::create_in(&tmp, "scratch", &next));
    assert!(tmp.is_dir());
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);

    let taken = tmp.join("scratch-0");
    std::fs::create_dir(&taken).unwrap();
    std::fs::write(taken.join("note.txt"), b"kept").unwrap();
    next.store(0, Ordering::Relaxed);
    let fresh = TempDir::create_in(&tmp, "scratch", &next);
    assert_eq!(fresh.0, tmp.join("scratch-1"));
    drop(fresh);
    assert!(!tmp.join("scratch-1").exists());
    assert_eq!(std::fs::read(taken.join("note.txt")).unwrap(), b"kept");
    assert!(tmp.is_dir() && root.0.is_dir());
}
