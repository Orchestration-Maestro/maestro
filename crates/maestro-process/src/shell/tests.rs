//! Command resolution using supplied platform observations.
use super::resolve_windows_executable;
use std::path::{Path, PathBuf};

#[test]
fn extensionless_commands_follow_directory_and_extension_order() {
    let directories = [PathBuf::from("current"), PathBuf::from("bin")];
    let files = [
        Path::new("current").join("custom.CMD"),
        Path::new("current").join("custom.EXE"),
        Path::new("bin").join("custom.COM"),
    ];
    for (extensions, expected) in [
        (".CMD;.EXE;.COM", "custom.CMD"),
        (".EXE;.CMD;.COM", "custom.EXE"),
        (".COM;.CMD;.EXE", "custom.CMD"),
    ] {
        assert_eq!(
            resolve_windows_executable("custom", &directories, extensions, |p| files
                .contains(&p.to_path_buf())),
            Some(Path::new("current").join(expected))
        );
    }
    assert_eq!(
        resolve_windows_executable("missing", &directories, ".CMD;.EXE", |p| files
            .contains(&p.to_path_buf())),
        None
    );
    assert_eq!(
        resolve_windows_executable("custom.CMD", &directories, ".EXE", |_| unreachable!()),
        Some(PathBuf::from("custom.CMD"))
    );
}

#[test]
fn explicit_command_directories_do_not_search_other_directories() {
    let command = Path::new("chosen dir").join("custom");
    let script = Path::new("chosen dir").join("custom.CMD");
    assert_eq!(
        resolve_windows_executable(
            command.to_str().unwrap(),
            &[PathBuf::from("other")],
            ".EXE;;.CMD",
            |p| p == script
        ),
        Some(script)
    );
}

#[test]
#[cfg(windows)]
fn verbatim_shell_commands_keep_environment_operands() {
    let output = super::windows_shell_launch("echo \"%MAESTRO_LITERAL_FILE%\"", None)
        .command()
        .unwrap()
        .env(
            "MAESTRO_LITERAL_FILE",
            "directory with spaces & literal.txt",
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        b"\"directory with spaces & literal.txt\"\r\n"
    );
}
