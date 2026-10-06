use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const BUILD_INPUTS: &[&str] = &[
    "PATH",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "MISE_DATA_DIR",
    "MISE_TRUSTED_CONFIG_PATHS",
    "RUSTC_WRAPPER",
    "CARGO_BUILD_JOBS",
    "CARGO_TARGET_DIR",
    "RUST_TEST_THREADS",
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_TEST_DEBUG",
    "SCCACHE_BASEDIRS",
    "SCCACHE_DIR",
    "SCCACHE_SERVER_PORT",
    "MAESTRO_BUILD_SLOT_DIR",
    "TERM",
    "LANG",
];

struct Scratch(PathBuf);

impl Scratch {
    fn create() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            // A fixed Unix base avoids accumulating nested temporary path prefixes.
            #[cfg(unix)]
            let base = PathBuf::from("/tmp");
            #[cfg(not(unix))]
            let base = std::env::temp_dir();
            let path = base.join(format!(
                "maestro-isolated-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("repository tools: create scratch: {error}")),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("repository tools: remove scratch: {error}");
        }
    }
}

fn prepare(command: &mut Command) -> Result<Scratch, String> {
    let scratch = Scratch::create()?;
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(std::ffi::OsStr::to_owned)))
        .collect::<Vec<_>>();
    command.env_clear();
    for name in BUILD_INPUTS {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    for (name, suffix) in [
        ("CARGO_HOME", ".cargo"),
        ("RUSTUP_HOME", ".rustup"),
        ("MISE_DATA_DIR", ".local/share/mise"),
    ] {
        if std::env::var_os(name).is_none()
            && let Some(home) = &home
        {
            command.env(name, home.join(suffix));
        }
    }
    for (name, directory) in [
        ("HOME", "home"),
        ("TMPDIR", "tmp"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_RUNTIME_DIR", "runtime"),
        ("XDG_CONFIG_DIRS", "config-dirs"),
        ("XDG_DATA_DIRS", "data-dirs"),
    ] {
        let path = scratch.0.join(directory);
        fs::create_dir(&path).map_err(|error| format!("repository tools: create root: {error}"))?;
        command.env(name, path);
    }
    command
        .env("TMP", scratch.0.join("tmp"))
        .env("TEMP", scratch.0.join("tmp"));
    command.env("MAESTRO_NO_LOCAL_LLM", "1");
    for (name, value) in explicit {
        if let Some(value) = value {
            command.env(name, value);
        } else {
            command.env_remove(name);
        }
    }
    Ok(scratch)
}

pub(super) fn run(mut command: Command) -> Result<u8, String> {
    let _scratch = prepare(&mut command)?;
    process::run(&mut command)
}

pub(super) fn output(mut command: Command) -> Result<std::process::Output, String> {
    let _scratch = prepare(&mut command)?;
    command
        .output()
        .map_err(|error| format!("repository tools: spawn child: {error}"))
}

#[cfg(unix)]
mod process {
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::Command;
    use std::sync::atomic::{AtomicI32, Ordering};

    static GROUP: AtomicI32 = AtomicI32::new(0);
    static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

    unsafe extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
        fn kill(pid: i32, number: i32) -> i32;
    }

    extern "C" fn interrupt(number: i32) {
        INTERRUPTED.store(number, Ordering::SeqCst);
        let group = GROUP.load(Ordering::SeqCst);
        if group > 0 {
            // kill is async-signal-safe; only the owned child group is addressed.
            unsafe {
                kill(-group, 9);
            }
        }
    }

    pub(super) fn run(command: &mut Command) -> Result<u8, String> {
        // The executable owns these handlers for its entire process lifetime.
        unsafe {
            signal(2, interrupt as *const () as usize);
            signal(15, interrupt as *const () as usize);
        }
        let mut child = command
            .process_group(0)
            .spawn()
            .map_err(|error| format!("repository tools: spawn child: {error}"))?;
        let group = i32::try_from(child.id()).map_err(|error| error.to_string())?;
        GROUP.store(group, Ordering::SeqCst);
        if INTERRUPTED.load(Ordering::SeqCst) != 0 {
            // A signal arriving during spawn still terminates the owned group.
            unsafe {
                kill(-group, 9);
            }
        }
        let status = child
            .wait()
            .map_err(|error| format!("repository tools: wait child: {error}"))?;
        // Reap the direct child, then stop any descendant that outlived it.
        unsafe {
            kill(-group, 9);
        }
        GROUP.store(0, Ordering::SeqCst);
        let interruption = INTERRUPTED.load(Ordering::SeqCst);
        let code = if interruption != 0 {
            128 + interruption
        } else {
            status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
        };
        Ok(u8::try_from(code).unwrap_or(1))
    }
}

#[cfg(not(unix))]
mod process {
    use std::process::Command;

    pub(super) fn run(command: &mut Command) -> Result<u8, String> {
        let status = command
            .status()
            .map_err(|error| format!("repository tools: spawn child: {error}"))?;
        Ok(status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1))
    }
}
