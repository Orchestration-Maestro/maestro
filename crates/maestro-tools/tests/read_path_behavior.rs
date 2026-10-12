//! Read-path transformations against the real filesystem and host lookups.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod read_support;

use maestro_tools::{
    ReadToolOptions, create_read_tool_definition, expand_path, resolve_read_path, resolve_to_cwd,
};
use read_support::*;

/// The host's home directory as the native lookup reports it.
fn home() -> String {
    std::env::home_dir().unwrap().to_str().unwrap().to_owned()
}

#[test]
fn path_expansion_preserves_only_the_named_transforms() {
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
    assert_eq!(expand_path("~").unwrap(), home());
    assert_eq!(expand_path("~/notes").unwrap(), format!("{}/notes", home()));
    assert_eq!(
        expand_path("@~/notes").unwrap(),
        format!("{}/notes", home())
    );
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
    for (mode, value) in [("home-empty", Some("")), ("home-absent", None)] {
        if isolated(test, mode, &[("HOME", value)]) {
            assert_eq!(expand_path("plain").unwrap(), "plain");
            let lookup = std::env::home_dir().map(|home| home.to_str().unwrap().to_owned());
            assert_eq!(
                expand_path("~/x").ok(),
                lookup.map(|home| format!("{home}/x"))
            );
            assert_ne!(expand_path("~/x").ok(), Some("/x".to_owned()));
        }
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
    let parent = TempDir::new();
    let taken = parent.0.join("scratch-0");
    std::fs::create_dir(&taken).unwrap();
    std::fs::write(taken.join("sentinel"), b"kept").unwrap();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let fresh = TempDir::create_in(&parent.0, "scratch", &next);
    assert_eq!(fresh.0, parent.0.join("scratch-1"));
    drop(fresh);
    assert_eq!(std::fs::read(taken.join("sentinel")).unwrap(), b"kept");
}
