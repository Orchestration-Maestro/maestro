//! fd operands, which differ between hosts only for a full-path glob.
use super::*;

/// The fixed flags before the mode-dependent operands.
const FLAGS: [&str; 8] = [
    "--color=never",
    "--hidden",
    "--no-require-git",
    "--max-results",
    "5",
    "--print0",
    "--exclude",
    ".git",
];

/// The complete operand list of a search in `mode`.
fn expected(mode: &str, tail: &[&str]) -> Vec<String> {
    [mode]
        .iter()
        .chain(&FLAGS)
        .chain(tail)
        .map(|argument| (*argument).to_owned())
        .collect()
}

#[test]
fn glob_operands_differ_only_for_windows_full_paths() {
    for windows in [false, true] {
        assert_eq!(
            arguments("/root", "*.rs", 5.0, windows),
            expected("--glob", &["--", "*.rs", "/root"])
        );
    }
    assert_eq!(
        arguments("/root", "src/*.rs", 5.0, false),
        expected("--glob", &["--full-path", "--", "**/src/*.rs", "/root"])
    );
    assert_eq!(
        arguments("/root", "**/*.rs", 5.0, false),
        expected("--glob", &["--full-path", "--", "**/*.rs", "/root"])
    );
    let windows = arguments(r"D:\root", "some/parent/**", 5.0, true);
    assert_eq!(
        windows,
        expected(
            "--regex",
            &["--full-path", "--", &windows[windows.len() - 2], r"D:\root"]
        )
    );
}

/// Full-path globs and `/`-spelled absolute paths, as fd receives them after the `**/` prefix.
const EQUIVALENCE: [(&str, &[&str]); 14] = [
    (
        "**/src/main.rs",
        &["D:/root/src/main.rs", "D:/root/src/main.rsx"],
    ),
    ("**/src/*.rs", &["D:/root/src/a.rs", "D:/root/src/a/b.rs"]),
    ("**/a?c", &["D:/x/abc", "D:/x/a/c"]),
    (
        "**/src/**/x.rs",
        &["D:/r/src/x.rs", "D:/r/src/a/b/x.rs", "D:/r/lib/x.rs"],
    ),
    ("**/a[/b]c", &["D:/r/a/c", "D:/r/abc", "D:/r/axc"]),
    ("**/a[!b]c", &["D:/r/a/c", "D:/r/axc", "D:/r/abc"]),
    (
        "**/a[!-/]c",
        &["D:/r/a/c", "D:/r/a.c", "D:/r/a[c", "D:/r/a]c"],
    ),
    ("**/a[+-0]c", &["D:/r/a/c", "D:/r/a.c", "D:/r/a1c"]),
    ("**/f[*].txt", &["D:/r/f*.txt", "D:/r/fx.txt"]),
    ("**/a.b+(c)^$", &["D:/r/a.b+(c)^$", "D:/r/axb+(c)^$"]),
    (
        "**/{src,lib}/*.rs",
        &["D:/r/lib/x.rs", "D:/r/src/x.rs", "D:/r/bin/x.rs"],
    ),
    (
        "D:/root/*.txt",
        &["D:/root/a.txt", "D:/root/sub/a.txt", "E:/root/a.txt"],
    ),
    (
        "**/share/*.txt",
        &["//server/share/a.txt", "//server/other/a.txt"],
    ),
    ("**/雪/.git/**", &["D:/r/雪/.git/x", "D:/r/雪/git/x"]),
];

/// The regex fd builds from `pattern`: bytes, with `.` matching a line break.
fn fd_regex(pattern: &str) -> regex::bytes::Regex {
    regex::bytes::RegexBuilder::new(pattern)
        .dot_matches_new_line(true)
        .build()
        .unwrap()
}

#[test]
fn windows_regex_answers_as_fd_glob_does_with_either_separator() {
    for (glob, paths) in EQUIVALENCE {
        let compiled = globset::GlobBuilder::new(glob)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .unwrap();
        let fd = fd_regex(compiled.regex());
        let windows = fd_regex(&either_separator(glob).unwrap());
        for path in paths {
            let expected = fd.is_match(path.as_bytes());
            let native = path.replace('/', "\\");
            assert_eq!(windows.is_match(path.as_bytes()), expected, "{glob} {path}");
            assert_eq!(
                windows.is_match(native.as_bytes()),
                expected,
                "{glob} {native}"
            );
        }
    }
}

#[test]
fn invalid_globs_keep_fd_glob_operands() {
    for glob in ["**/x/[a", "**/x/{a,b"] {
        assert_eq!(either_separator(glob), None, "{glob}");
        let operands = arguments(r"D:\root", &glob[3..], 5.0, true);
        assert_eq!(operands[0], "--glob", "{glob}");
        assert_eq!(operands[operands.len() - 2], glob, "{glob}");
    }
}
