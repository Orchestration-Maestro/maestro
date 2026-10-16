//! Locked native browser-author preparation and command dispatch.

use std::{
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// Authored fixture relative to the checkout, never copied during preparation.
const FIXTURE: &str = "crates/maestro-tooling/tests/fixtures/browser_author";

/// Prepared native tools and route-owned configuration.
struct Prepared {
    /// Generated presentation consumed by the fixture compiler.
    inputs: PathBuf,
    /// Absolute-path configuration consumed by Trunk.
    config: PathBuf,
    /// Locked metadata adapter executable.
    adapter: PathBuf,
}

/// Enter the shared preparation path or forward the adapter's literal operands.
pub(crate) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let (operation, args) = args
        .split_first()
        .ok_or_else(|| io::Error::other("missing browser operation"))?;
    if operation == "cargo" {
        return forward(args, cargo);
    }
    let operation = operation
        .to_str()
        .ok_or_else(|| io::Error::other("invalid browser operation"))?;
    let route = match operation {
        "check" | "library-build" | "watch" => "library",
        "example-check" | "example-build" | "serve" => "example",
        "debug-build" => "debug",
        _ => return Err(io::Error::other("unknown browser operation")),
    };
    let pack = super::argument(args, 0, "pack")?;
    let pack = if pack.is_absolute() {
        pack.to_path_buf()
    } else {
        checkout.join(pack)
    };
    let mode = args
        .get(1)
        .ok_or_else(|| io::Error::other("missing mode parameter"))?;
    if operation == "check" {
        let status = format_and_check(cargo, checkout)?;
        if status != ExitCode::SUCCESS {
            return Ok(status);
        }
    }
    let prepared = match prepare(cargo, checkout, route, &pack, mode)? {
        Ok(prepared) => prepared,
        Err(status) => return Ok(status),
    };
    if matches!(operation, "check" | "example-check") {
        let status = fixture_check(operation, cargo, checkout, &prepared)?;
        if status != ExitCode::SUCCESS || operation == "example-check" {
            return Ok(status);
        }
        let mut next = vec!["example-check".into()];
        next.extend_from_slice(args);
        return run(&next, cargo, checkout);
    }
    trunk(operation, &args[2..], cargo, checkout, &prepared)
}

/// Compile the authored fixture against its adjacent lock and prepared includes.
fn fixture_check(
    operation: &str,
    cargo: &Path,
    checkout: &Path,
    prepared: &Prepared,
) -> io::Result<ExitCode> {
    let mut command = Command::new(cargo);
    command
        .current_dir(checkout)
        .args([
            if operation == "check" {
                "clippy"
            } else {
                "check"
            },
            "--locked",
            "--manifest-path",
        ])
        .arg(checkout.join(FIXTURE).join("Cargo.toml"))
        .args(["--target", "wasm32-unknown-unknown"])
        .env("MAESTRO_BROWSER_INPUTS", &prepared.inputs);
    if operation == "check" {
        command.args(["--", "-D", "warnings"]);
    }
    command.status().map(super::exit_code)
}

/// Keep formatter repair explicit and check the actual browser library strictly.
fn format_and_check(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    for arguments in [
        vec![OsString::from("fmt"), "-p".into(), "maestro-web".into()],
        vec![
            "fmt".into(),
            "--manifest-path".into(),
            checkout.join(FIXTURE).join("Cargo.toml").into_os_string(),
        ],
        [
            "clippy",
            "--no-deps",
            "-p",
            "maestro-web",
            "--target",
            "wasm32-unknown-unknown",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]
        .map(Into::into)
        .to_vec(),
    ] {
        let status = super::exit_code(
            Command::new(cargo)
                .current_dir(checkout)
                .args(arguments)
                .status()?,
        );
        if status != ExitCode::SUCCESS {
            return Ok(status);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Preserve native Cargo output, ordering, and status while locking metadata once.
fn forward(args: &[OsString], cargo: &Path) -> io::Result<ExitCode> {
    let mut command = Command::new(cargo);
    command.args(args);
    if args.first().is_some_and(|arg| arg == "metadata")
        && !args.iter().any(|arg| arg == "--locked")
    {
        command.arg("--locked");
    }
    command.status().map(super::exit_code)
}

/// Build both native owners before exporting presentation and generating route configuration.
fn prepare(
    cargo: &Path,
    checkout: &Path,
    route: &str,
    pack: &Path,
    mode: &OsStr,
) -> io::Result<Result<Prepared, ExitCode>> {
    let status = bootstrap(cargo, checkout)?;
    if status != ExitCode::SUCCESS {
        return Ok(Err(status));
    }
    let metadata = super::locked_metadata(cargo, checkout)?;
    let native = metadata.target_directory.join("debug").into_std_path_buf();
    let directory = checkout.join("target/maestro-browser/.author").join(route);
    let inputs = directory.join("inputs");
    fs::create_dir_all(&inputs)?;
    let status = super::exit_code(
        Command::new(
            native
                .join("examples/browser_author_inputs")
                .with_extension(std::env::consts::EXE_EXTENSION),
        )
        .current_dir(checkout)
        .arg(pack)
        .arg(mode)
        .arg(&inputs)
        .status()?,
    );
    if status != ExitCode::SUCCESS {
        return Ok(Err(status));
    }
    let config = directory.join("Trunk.json");
    let prepared = Prepared {
        inputs,
        config,
        adapter: native
            .join("browser-cargo-locked")
            .with_extension(std::env::consts::EXE_EXTENSION),
    };
    configuration(cargo, checkout, route, (pack, mode), &prepared)?;
    Ok(Ok(prepared))
}

/// Bootstrap recipe users' native artifacts from the locked root workspace.
fn bootstrap(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    for args in [
        [
            "build",
            "--locked",
            "-p",
            "maestro-theme",
            "--example",
            "browser_author_inputs",
        ],
        [
            "build",
            "--locked",
            "-p",
            "maestro-tooling",
            "--bin",
            "browser-cargo-locked",
        ],
    ] {
        let status = super::exit_code(
            Command::new(cargo)
                .current_dir(checkout)
                .args(args)
                .arg("--manifest-path")
                .arg(checkout.join("Cargo.toml"))
                .status()?,
        );
        if status != ExitCode::SUCCESS {
            return Ok(status);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Serialize native Trunk configuration without shell reconstruction or alternate source trees.
fn configuration(
    cargo: &Path,
    checkout: &Path,
    route: &str,
    identity: (&Path, &OsStr),
    prepared: &Prepared,
) -> io::Result<()> {
    let (pack, mode) = identity;
    let inputs = &prepared.inputs;
    let fixture = checkout.join(FIXTURE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("Trunk.json"))?)?;
    let target = if route == "debug" {
        "debug.html"
    } else {
        "index.html"
    };
    value["build"]["target"] = serde_json::to_value(fixture.join(target))?;
    value["build"]["dist"] =
        serde_json::to_value(checkout.join("target/maestro-browser").join(route))?;
    let mut roots: Vec<PathBuf> =
        serde_json::from_slice(&fs::read(inputs.join("watch-dirs.json"))?)?;
    roots.extend([fixture, checkout.join("assets/brand")]);
    value["watch"]["watch"] = serde_json::to_value(roots)?;
    let manifest = checkout.join("Cargo.toml");
    let arguments = [
        OsStr::new("run"),
        OsStr::new("--locked"),
        OsStr::new("--manifest-path"),
        manifest.as_os_str(),
        OsStr::new("-p"),
        OsStr::new("maestro-theme"),
        OsStr::new("--example"),
        OsStr::new("browser_author_inputs"),
        OsStr::new("--"),
        pack.as_os_str(),
        mode,
        inputs.as_os_str(),
    ];
    let arguments = arguments
        .iter()
        .map(|arg| {
            arg.to_str()
                .ok_or_else(|| io::Error::other("Trunk configuration requires UTF-8 paths"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    value["hooks"] = serde_json::json!([{
        "stage": "pre_build", "command": cargo, "command_arguments": arguments
    }]);
    fs::write(&prepared.config, serde_json::to_vec_pretty(&value)?)
}

/// Start Trunk as the sole build/watch/server owner with a child-local metadata guard.
fn trunk(
    operation: &str,
    args: &[OsString],
    cargo: &Path,
    checkout: &Path,
    prepared: &Prepared,
) -> io::Result<ExitCode> {
    let action = match operation {
        "serve" => "serve",
        "watch" => "watch",
        _ => "build",
    };
    let mut command = Command::new("trunk");
    command
        .current_dir(checkout)
        .arg(action)
        .arg("--config")
        .arg(&prepared.config)
        .arg("--locked")
        .env("MAESTRO_BROWSER_INPUTS", &prepared.inputs)
        .env("MAESTRO_BROWSER_CARGO", cargo)
        .env("CARGO", &prepared.adapter);
    if operation != "debug-build" {
        command.arg("--release");
    }
    command.args(args).status().map(super::exit_code)
}
