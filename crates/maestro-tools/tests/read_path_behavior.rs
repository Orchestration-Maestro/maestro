//! Read-path transformations against the real filesystem and host lookups.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::ls_support::posix_spelling;
use crate::read_support;
use crate::runtime::run;

use maestro_tools::{
    ReadToolOptions, create_read_tool_definition, expand_path, resolve_read_path, resolve_to_cwd,
};
use read_support::*;
#[cfg(unix)]
use std::sync::Arc;

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
        assert_eq!(
            posix_spelling(&resolve_to_cwd(path, cwd).unwrap()),
            posix_spelling(expected),
            "{path:?}"
        );
    }
    let process = std::env::current_dir().unwrap();
    assert_eq!(
        posix_spelling(&resolve_to_cwd("leaf", "relative").unwrap()),
        posix_spelling(&format!("{}/relative/leaf", process.to_str().unwrap()))
    );
}

/// A canonically equivalent pair, precomposed then decomposed.
const EQUIVALENT: [&str; 2] = ["\u{e9}", "e\u{301}"];

/// Whether this host folds the canonically equivalent pair onto one entry.
fn folds_equivalents() -> bool {
    let scratch = Scratch::new().unwrap();
    !scratch
        .keeps_distinct(&EQUIVALENT.map(std::ffi::OsStr::new))
        .unwrap()
}

/// Whether a case stores a decomposed name and asks without the combining mark on a host that
/// folds canonically equivalent names. That host finds the stored file under the asked
/// spelling itself, so the case reports its premise instead.
fn reaches_decomposed(index: usize, files: &[&str], ask: &str) -> bool {
    let reaches = files.iter().any(|file| file.contains('\u{301}'))
        && !ask.contains('\u{301}')
        && folds_equivalents();
    if reaches {
        eprintln!("case {index} needs a host that keeps {EQUIVALENT:?} apart");
    }
    reaches
}

/// Files present, the spelling asked for, and the existing file that spelling must select.
type Fallback = (&'static [&'static str], &'static str, &'static str);

/// A file name whose meridiem follows a narrow no-break space.
const AM: &str = "shot 10.00\u{202f}AM.png";

#[test]
fn read_paths_try_original_ampm_nfd_curly_then_combined() {
    let cases: [Fallback; 16] = [
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
        (
            &["x\u{a0}y 1\u{202f}AM.png"],
            "x\u{a0}y 1 AM.png",
            "x\u{a0}y 1\u{202f}AM.png",
        ),
        (
            &["x y 1\u{202f}AM.png"],
            "x\u{a0}y 1 AM.png",
            "x y 1\u{202f}AM.png",
        ),
    ];
    for (index, (files, ask, found)) in cases.into_iter().enumerate() {
        if reaches_decomposed(index, files, ask) {
            continue;
        }
        let dir = Scratch::new().unwrap();
        for file in files {
            dir.file(file, b"");
        }
        let resolved = resolve_read_path(ask, &dir.text()).unwrap();
        let expected = dir.path().join(found);
        assert_eq!(resolved, expected.to_string_lossy(), "case {index}");
    }
    let dir = Scratch::new().unwrap();
    std::fs::create_dir(dir.path().join("folder")).unwrap();
    assert_eq!(
        resolve_read_path("folder", &dir.text()).unwrap(),
        dir.path().join("folder").to_string_lossy()
    );
}

/// A path naming an existing file resolves to that file under the spelling it was written with.
#[test]
fn read_paths_prefer_the_written_spelling_of_an_existing_file() {
    let dir = Scratch::new().unwrap();
    dir.file("x\u{a0}y.txt", b"written");
    dir.file("p\u{a0}q.txt", b"written pair");
    dir.file("p q.txt", b"plain pair");
    let cwd = dir.text();
    let written = format!("{cwd}/x\u{a0}y.txt");
    for ask in [
        "x\u{a0}y.txt",
        "@x\u{a0}y.txt",
        &written,
        "./sub/../x\u{a0}y.txt",
    ] {
        assert_eq!(
            posix_spelling(&resolve_read_path(ask, &cwd).unwrap()),
            posix_spelling(&written),
            "{ask:?}"
        );
    }
    assert_eq!(
        posix_spelling(&resolve_read_path("p\u{a0}q.txt", &cwd).unwrap()),
        posix_spelling(&format!("{cwd}/p\u{a0}q.txt"))
    );
    let home = Scratch::new().unwrap();
    home.file("h\u{a0}o.txt", b"home written");
    let home_text = home.text();
    let envs = [
        ("HOME", Some(home_text.as_str())),
        ("USERPROFILE", Some(home_text.as_str())),
    ];
    if isolated(
        "read_paths_prefer_the_written_spelling_of_an_existing_file",
        "home-written",
        &envs,
    ) {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            posix_spelling(&resolve_read_path("~/h\u{a0}o.txt", &cwd).unwrap()),
            posix_spelling(&format!("{home}/h\u{a0}o.txt"))
        );
    }
}

/// A typed path whose Unicode spaces stand for plain spaces still finds its file.
#[test]
fn read_paths_fall_back_to_the_normalized_spelling_when_nothing_has_the_written_one() {
    let dir = Scratch::new().unwrap();
    dir.file("sp ace.txt", b"plain");
    let cwd = dir.text();
    for ask in ["sp\u{a0}ace.txt", "@sp\u{a0}ace.txt"] {
        assert_eq!(
            posix_spelling(&resolve_read_path(ask, &cwd).unwrap()),
            posix_spelling(&format!("{cwd}/sp ace.txt")),
            "{ask:?}"
        );
    }
    assert_eq!(
        posix_spelling(&resolve_read_path("n\u{a0}e.txt", &cwd).unwrap()),
        posix_spelling(&format!("{cwd}/n e.txt"))
    );
}

#[test]
fn read_paths_accept_lowercase_meridiem() {
    for suffix in ["am", "pm"] {
        let dir = Scratch::new().unwrap();
        let chosen = format!("Screenshot 2024-01-01 at 10.00.00\u{202f}{suffix}.png");
        dir.file(&chosen, b"chosen");
        let ask = format!("Screenshot 2024-01-01 at 10.00.00 {suffix}.png");
        let resolved = resolve_read_path(&ask, &dir.text()).unwrap();
        assert_eq!(
            posix_spelling(&resolved),
            posix_spelling(&format!("{}/{chosen}", dir.text()))
        );
        assert_eq!(std::fs::read(&resolved).unwrap(), b"chosen");
    }
}

#[cfg(unix)]
#[test]
fn read_paths_keep_lexical_symlink_identity() {
    let dir = Scratch::new().unwrap();
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    dir.file("f", b"top");
    dir.file("a/f", b"under a");
    std::os::unix::fs::symlink(dir.path().join("a/b"), dir.path().join("link")).unwrap();
    let cwd = dir.text();
    assert_eq!(
        posix_spelling(&resolve_read_path("link", &cwd).unwrap()),
        posix_spelling(&format!("{cwd}/link"))
    );
    assert_eq!(
        posix_spelling(&resolve_read_path("link/../f", &cwd).unwrap()),
        posix_spelling(&format!("{cwd}/f"))
    );
    let tool = create_read_tool_definition::<()>(cwd, ReadToolOptions::default());
    let result = call(&tool, input("link/../f", None, None), None, None).unwrap();
    assert_eq!(only_text(&result), "top");
}

#[test]
fn native_path_host_lookups_are_lazy() {
    let test = "native_path_host_lookups_are_lazy";
    #[cfg(unix)]
    if isolated(test, "cwd", &[]) {
        let parent = maestro_test_directory::Scratch::new().unwrap();
        parent.enter_removed("gone").unwrap();
        assert!(std::env::current_dir().is_err());
        assert_eq!(
            resolve_to_cwd("/abs/file", "relative").unwrap(),
            "/abs/file"
        );
        assert_eq!(resolve_to_cwd("rel", "/base").unwrap(), "/base/rel");
        assert_eq!(expand_path("plain").unwrap(), "plain");
        assert!(resolve_to_cwd("rel", "relative-cwd").is_err());
    }
    let explicit = Some("/explicit");
    if isolated(
        test,
        "home-explicit",
        &[("HOME", explicit), ("USERPROFILE", explicit)],
    ) {
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
        assert_eq!(expand_path("~/x").unwrap(), "x");
        assert_eq!(expand_path("~/").unwrap(), "");
        assert_eq!(expand_path("~").unwrap(), "");
    }
}

/// A tools `~` path keeps its written suffix exactly, separators and parent segments included.
#[test]
fn tools_tilde_paths_keep_the_written_suffix() {
    let envs = [("HOME", Some("/h")), ("USERPROFILE", Some("/h"))];
    if !isolated("tools_tilde_paths_keep_the_written_suffix", "rows", &envs) {
        return;
    }
    for (written, expected) in [
        ("~", "/h"),
        ("~/", "/h/"),
        ("~//note", "/h//note"),
        ("~/a/../f", "/h/a/../f"),
        ("~//x", "/h//x"),
        ("~child", "~child"),
    ] {
        assert_eq!(expand_path(written).unwrap(), expected, "{written:?}");
    }
}

/// A tools `~` path keeps the `/` its author wrote, on every platform including Windows.
#[test]
fn tools_tilde_paths_keep_the_written_separator() {
    let envs = [("HOME", Some("/h")), ("USERPROFILE", Some("/h"))];
    if !isolated(
        "tools_tilde_paths_keep_the_written_separator",
        "separator",
        &envs,
    ) {
        return;
    }
    let expanded = expand_path("~/notes").unwrap();
    assert_eq!(expanded, "/h/notes");
    assert!(!expanded.contains('\\'));
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
    let dir = Scratch::new().unwrap();
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
    let (original, decoy) = (Scratch::new().unwrap(), Scratch::new().unwrap());
    original.file("f.txt", b"original");
    decoy.file("f.txt", b"decoy");
    std::env::set_current_dir(original.path()).unwrap();
    let tool = create_read_tool_definition::<()>(".".into(), ReadToolOptions::default());
    let pending = (tool.execute)("id".into(), input("f.txt", None, None), None, None, None);
    std::env::set_current_dir(decoy.path()).unwrap();
    assert_eq!(only_text(&run(pending).unwrap()), "original");
    let fresh = call(&tool, input("f.txt", None, None), None, None).unwrap();
    assert_eq!(only_text(&fresh), "decoy");
}

/// A working directory text cannot hold is refused instead of replaced.
#[cfg(unix)]
#[test]
fn resolve_rejects_a_working_directory_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    if isolated(
        "resolve_rejects_a_working_directory_that_is_not_utf8",
        "bytes",
        &[],
    ) {
        let dir = Scratch::new().unwrap();
        let name = <std::ffi::OsStr as OsStrExt>::from_bytes(b"cwd-\xff");
        if !dir.keeps_distinct(&[name]).unwrap() {
            eprintln!("the working directory needs a host that stores the name bytes {name:?}");
            return;
        }
        let raw = dir.path().join(name);
        std::fs::create_dir(&raw).unwrap();
        std::env::set_current_dir(&raw).unwrap();
        let error = resolve_to_cwd("x.txt", ".").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), "working directory is not valid UTF-8");
    }
}
