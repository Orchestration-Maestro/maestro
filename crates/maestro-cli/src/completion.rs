//! Effect-free shell completion output from the shared command metadata.

use crate::workspace_commands::spec;
use clap_complete::{Generator, Shell};
use std::io::{self, Write};

/// Shells supported by the command-line completion generator.
pub const SUPPORTED_SHELLS: [&str; 5] = ["bash", "elvish", "fish", "powershell", "zsh"];

/// Generate completion output for the one shell named after `completion`.
///
/// # Errors
/// Returns output failures unchanged.
pub fn run_completion(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    let [name] = args else {
        write_usage(stderr)?;
        return Ok(2);
    };
    if matches!(name.as_str(), "help" | "--help" | "-h") {
        write_usage(stderr)?;
        return Ok(0);
    }
    let Ok(shell) = name.parse::<Shell>() else {
        writeln!(stderr, "unknown shell: {name}")?;
        write_usage(stderr)?;
        return Ok(2);
    };
    let mut command = spec::command().bin_name("maestro");
    command.build();
    // Some generator helpers panic on write failure even through their fallible API.
    let mut bytes = Vec::new();
    shell.try_generate(&command, &mut bytes)?;
    if shell == Shell::Zsh {
        let script = String::from_utf8(bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        stdout.write_all(space_separated_zsh_options(&script).as_bytes())?;
    } else {
        stdout.write_all(&bytes)?;
    }
    Ok(0)
}

/// Present the supported shell spellings without starting any runtime.
fn write_usage(output: &mut dyn Write) -> io::Result<()> {
    writeln!(
        output,
        "usage: maestro completion <{}>",
        SUPPORTED_SHELLS.join("|")
    )
}

/// Keep generated zsh options compatible with space-separated CLI arguments.
fn space_separated_zsh_options(script: &str) -> String {
    let mut output = String::with_capacity(script.len());
    let mut rest = script;
    while let Some(start) = rest.find("--") {
        output.push_str(&rest[..start]);
        output.push_str("--");
        let candidate = &rest[start + 2..];
        let name_len = candidate
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
            .count();
        if name_len > 0 && candidate[name_len..].starts_with("=[") {
            output.push_str(&candidate[..name_len]);
            output.push('[');
            rest = &candidate[name_len + 2..];
        } else {
            rest = candidate;
        }
    }
    output.push_str(rest);
    output
}
