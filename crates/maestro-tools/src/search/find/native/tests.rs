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

#[test]
fn windows_full_path_regex_matches_either_separator() {
    let operands = arguments(r"D:\root", "src/**/[a/]*.spec.ts", 5.0, true);
    let regex = regex::bytes::RegexBuilder::new(&operands[operands.len() - 2])
        .dot_matches_new_line(true)
        .build()
        .unwrap();
    for path in [
        r"D:\root\src\a.spec.ts",
        r"D:\root\src\deep\nested\ab.spec.ts",
        "D:/root/src/deep/a.spec.ts",
        r"D:\root\x\src/deep\a.spec.ts",
    ] {
        assert!(regex.is_match(path.as_bytes()), "{path}");
    }
    for path in [
        r"D:\root\src\b.spec.ts",
        r"D:\root\src\a.spec.tsx",
        r"D:\root\other\a.spec.ts",
    ] {
        assert!(!regex.is_match(path.as_bytes()), "{path}");
    }
}
