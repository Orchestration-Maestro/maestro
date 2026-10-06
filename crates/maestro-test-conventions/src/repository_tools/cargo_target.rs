use super::cargo_directory::{end, field, string};
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

pub(super) fn run(root: &Path, args: Vec<OsString>) -> Result<u8, String> {
    let executable = args
        .first()
        .ok_or("repository tools: expected a Cargo artifact")?;
    let path = Path::new(executable);
    if path
        .parent()
        .and_then(Path::file_name)
        .is_none_or(|directory| directory != "deps")
    {
        return Command::new(executable)
            .args(&args[1..])
            .status()
            .map(super::status_code)
            .map_err(|error| format!("repository tools: launch program: {error}"));
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('-'))
        .filter(|(_, hash)| !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(|(name, _)| name);
    let mut metadata = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    metadata
        .args(["metadata", "--format-version=1", "--no-deps", "--locked"])
        .current_dir(root);
    let output = super::isolation::output(metadata)?;
    if !output.status.success() {
        return Err(format!(
            "repository tools: unknown test artifact: {}",
            path.display()
        ));
    }
    let json = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let mut owner = None;
    for package in objects(field(&json, "packages").unwrap_or("[]")) {
        let package_name = string(field(package, "name").unwrap_or(""));
        for target in objects(field(package, "targets").unwrap_or("[]")) {
            if name.is_some_and(|name| {
                string(field(target, "name").unwrap_or("")).replace('-', "_") == name
            }) && field(target, "test") == Some("true")
            {
                owner = Some(package_name.to_owned());
            }
        }
    }
    let owner = owner.ok_or_else(|| {
        format!(
            "repository tools: unknown test artifact: {}",
            path.display()
        )
    })?;
    let mut command = Command::new(executable);
    command.args(&args[1..]);
    runtime_environment(&mut command);
    if matches!(
        owner.as_str(),
        "maestro-models" | "maestro-agent" | "maestro-app"
    ) {
        timed_cases(root, executable, &args[1..])
    } else {
        super::isolation::run(command)
    }
}

pub(super) fn runtime_environment(command: &mut Command) {
    for name in [
        "LD_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "CARGO_MANIFEST_DIR",
        "CARGO_MANIFEST_PATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn timed_cases(root: &Path, executable: &OsString, args: &[OsString]) -> Result<u8, String> {
    if args.iter().any(|arg| arg == "--list") {
        let mut command = Command::new(executable);
        command.args(args);
        runtime_environment(&mut command);
        return super::isolation::run(command);
    }
    if args
        .iter()
        .any(|arg| arg.to_str().is_some_and(|arg| arg.starts_with("--logfile")))
    {
        return Err("repository tools: per-case execution does not support --logfile".into());
    }
    let list = |options: &[OsString]| -> Result<Vec<String>, String> {
        let mut command = Command::new(executable);
        command.args(options).arg("--list");
        runtime_environment(&mut command);
        let output = super::isolation::output(command)?;
        if !output.status.success() {
            use std::io::Write;
            std::io::stderr()
                .write_all(&output.stderr)
                .map_err(|error| error.to_string())?;
            return Err("repository tools: test selection failed".into());
        }
        let text = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        Ok(text
            .lines()
            .filter_map(|line| {
                line.strip_suffix(": test")
                    .or_else(|| line.strip_suffix(": benchmark"))
            })
            .map(str::to_owned)
            .collect())
    };
    let selected = list(args)?;
    let total = list(&[])?.len();
    let mut options = Vec::new();
    let mut threads = None;
    let mut cursor = 0;
    while cursor < args.len() {
        let text = args[cursor]
            .to_str()
            .ok_or("repository tools: invalid libtest option")?;
        if text == "--exact" {
            cursor += 1;
            continue;
        }
        if text.starts_with('-') {
            options.push(args[cursor].clone());
            if [
                "--skip",
                "--test-threads",
                "--color",
                "--format",
                "--shuffle-seed",
                "-Z",
            ]
            .contains(&text)
            {
                cursor += 1;
                let value = args
                    .get(cursor)
                    .ok_or("repository tools: missing libtest option value")?;
                if text == "--test-threads" {
                    threads = value.to_str().and_then(|value| value.parse::<usize>().ok());
                }
                options.push(value.clone());
            } else if let Some(value) = text.strip_prefix("--test-threads=") {
                threads = value.parse::<usize>().ok();
            }
        }
        cursor += 1;
    }
    let threads = threads
        .or_else(|| {
            std::env::var("RUST_TEST_THREADS")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
        });
    if threads == 0 {
        return Err("repository tools: test thread count must be positive".into());
    }
    let selected_count = selected.len();
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(selected));
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.min(selected_count) {
            let queue = &queue;
            let results = &results;
            let options = &options;
            scope.spawn(move || {
                loop {
                    let name = queue.lock().unwrap().pop_front();
                    let Some(name) = name else {
                        break;
                    };
                    let result = run_case(root, executable, &name, options);
                    results.lock().unwrap().push((name, result));
                }
            });
        }
    });
    let mut passed = 0;
    let mut failed = 0;
    let mut ignored = 0;
    use std::io::Write;
    for (name, result) in results.into_inner().unwrap() {
        match result {
            Ok((output, expired, case_ignored)) => {
                std::io::stdout()
                    .write_all(&output.stdout)
                    .map_err(|error| error.to_string())?;
                std::io::stderr()
                    .write_all(&output.stderr)
                    .map_err(|error| error.to_string())?;
                if expired {
                    eprintln!("repository tools: test timed out: {name}");
                    failed += 1;
                } else if !output.status.success() {
                    failed += 1;
                } else if case_ignored {
                    ignored += 1;
                } else {
                    passed += 1;
                }
            }
            Err(error) => {
                eprintln!("{error}");
                failed += 1;
            }
        }
    }
    println!(
        "test result: {}; {passed} passed; {failed} failed; {ignored} ignored; 0 measured; {} filtered out",
        if failed == 0 { "ok" } else { "FAILED" },
        total.saturating_sub(selected_count)
    );
    Ok(if failed == 0 { 0 } else { 1 })
}

fn run_case(
    root: &Path,
    executable: &OsString,
    name: &str,
    options: &[OsString],
) -> Result<(std::process::Output, bool, bool), String> {
    let scratch = super::isolation::Scratch::create()?;
    let outcome = scratch.0.join("outcome");
    let mut command = Command::new(std::env::current_exe().map_err(|error| error.to_string())?);
    command
        .arg("--root")
        .arg(root)
        .arg("cargo-case")
        .arg(executable)
        .arg("--exact")
        .arg(name)
        .args(options)
        .arg("--logfile")
        .arg(&outcome);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("repository tools: spawn test case: {error}"))?;
    let pid = child.id();
    let start = std::time::Instant::now();
    let deadline = deadline();
    let (send, receive) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let output = child.wait_with_output();
        let elapsed = start.elapsed();
        let _ = send.send((output, elapsed));
    });
    let (result, expired) = match receive.recv_timeout(deadline) {
        Ok((output, elapsed)) => (output, elapsed >= deadline),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .status();
            }
            let (output, _) = receive.recv().map_err(|error| error.to_string())?;
            (output, true)
        }
        Err(error) => return Err(error.to_string()),
    };
    waiter
        .join()
        .map_err(|_| "repository tools: case waiter panicked")?;
    let output = result.map_err(|error| format!("repository tools: wait test case: {error}"))?;
    let ignored = if output.status.success() && !expired {
        let outcome = std::fs::read_to_string(&outcome)
            .map_err(|error| format!("repository tools: read test outcome: {error}"))?;
        if outcome.starts_with("ok ") {
            false
        } else if outcome.starts_with("ignored ") || outcome.starts_with("ignored: ") {
            true
        } else {
            return Err("repository tools: unknown test outcome".into());
        }
    } else {
        false
    };
    Ok((output, expired, ignored))
}

fn deadline() -> std::time::Duration {
    #[cfg(test)]
    if let Ok(value) = std::env::var("MAESTRO_FIXTURE_DEADLINE_MS") {
        return std::time::Duration::from_millis(value.parse().expect("fixture deadline"));
    }
    std::time::Duration::from_millis(30_000)
}

fn objects(array: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut cursor = 1;
    while cursor < array.len() {
        while array
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b',')
        {
            cursor += 1;
        }
        if array.as_bytes().get(cursor) != Some(&b'{') {
            break;
        }
        let next = end(array, cursor);
        objects.push(&array[cursor..next]);
        cursor = next;
    }
    objects
}
