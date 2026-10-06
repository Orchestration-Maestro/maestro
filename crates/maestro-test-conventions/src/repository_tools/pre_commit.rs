use std::process::Command;

pub(super) fn run() -> Result<u8, String> {
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only", "-z"])
        .output()
        .map_err(|error| format!("repository tools: capture staged paths: {error}"))?;
    if !staged.status.success() {
        return Ok(super::status_code(staged.status));
    }
    let selected = staged.stdout.split(|byte| *byte == 0).any(|path| {
        path.starts_with(b"crates/maestro-models/")
            || path.starts_with(b"crates/maestro-web/")
            || path == b"Cargo.toml"
            || path == b"Cargo.lock"
    });
    println!("Running formatting, linting, and type checking...");
    let code = super::format_staged::run()?;
    let code = if code == 0 {
        Command::new("just")
            .arg("check")
            .status()
            .map(super::status_code)
            .map_err(|error| error.to_string())?
    } else {
        code
    };
    if code != 0 {
        println!("❌ Checks failed. Please fix the errors before committing.");
        return Ok(code);
    }
    let browser = std::fs::read_to_string(".github/ci.toml")
        .unwrap_or_default()
        .lines()
        .any(|line| line.trim() == "browser_build = true");
    if browser && selected {
        println!("Running browser smoke check...");
        let code = Command::new("just")
            .arg("browser-smoke")
            .status()
            .map(super::status_code)
            .map_err(|error| error.to_string())?;
        if code != 0 {
            println!("❌ Browser smoke check failed.");
            return Ok(code);
        }
    }
    println!("✅ All pre-commit checks passed!");
    Ok(0)
}
