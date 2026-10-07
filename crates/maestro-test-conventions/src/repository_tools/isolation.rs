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

pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn create() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            // A fixed Unix base avoids accumulating nested temporary path prefixes.
            #[cfg(unix)]
            let base = std::env::var_os("MAESTRO_CASE_SCRATCH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp"));
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
    for name in ["MAESTRO_CASE_SCRATCH", "MAESTRO_CASE_GROUP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
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
    process::output(&mut command)
}

pub(super) fn interrupted() -> bool {
    process::interrupted()
}

pub(super) fn case_output(
    mut command: Command,
    deadline: std::time::Duration,
) -> Result<(std::process::Output, bool), String> {
    let scratch = prepare(&mut command)?;
    command
        .env("MAESTRO_CASE_SCRATCH", &scratch.0)
        .env("MAESTRO_CASE_GROUP", "1");
    process::case_output(&mut command, deadline)
}

#[cfg(unix)]
mod process {
    use std::collections::BTreeMap;
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command, Output, Stdio};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    static INTERRUPTED: AtomicI32 = AtomicI32::new(0);
    static WAKE: AtomicI32 = AtomicI32::new(-1);
    static OWNERS: OnceLock<Mutex<BTreeMap<i32, Owner>>> = OnceLock::new();

    struct Owner {
        leader: bool,
        timed: bool,
    }

    unsafe extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
        fn kill(pid: i32, number: i32) -> i32;
        fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
        #[cfg(target_os = "linux")]
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        #[cfg(target_os = "linux")]
        fn prctl(option: i32, ...) -> i32;
    }

    extern "C" fn interrupt(number: i32) {
        INTERRUPTED.store(number, Ordering::SeqCst);
        // Wake the registry owner without taking a lock in the signal handler.
        unsafe {
            write(WAKE.load(Ordering::SeqCst), &1u8, 1);
        }
    }

    fn owners() -> &'static Mutex<BTreeMap<i32, Owner>> {
        OWNERS.get_or_init(|| {
            let (mut read, write) = UnixStream::pair().expect("interrupt channel");
            WAKE.store(write.as_raw_fd(), Ordering::SeqCst);
            std::thread::spawn(move || {
                let _write = write;
                let mut byte = [0];
                while read.read_exact(&mut byte).is_ok() {
                    let owners = owners().lock().unwrap();
                    for (&pid, owner) in owners.iter() {
                        cancel(pid, owner);
                    }
                }
            });
            unsafe {
                signal(2, interrupt as *const () as usize);
                signal(15, interrupt as *const () as usize);
            }
            Mutex::new(BTreeMap::new())
        })
    }

    fn cancel(pid: i32, owner: &Owner) {
        let target = if owner.leader { -pid } else { pid };
        unsafe {
            kill(target, 15);
            if owner.timed {
                kill(target, 9);
            }
        }
    }

    fn spawn(command: &mut Command, timed: bool) -> Result<Child, String> {
        let mut owners = owners().lock().unwrap();
        let leader = std::env::var_os("MAESTRO_CASE_GROUP").is_none();
        if leader {
            command.process_group(0);
        }
        #[cfg(target_os = "linux")]
        if timed {
            // Adopt descendants so cancelling a whole group also reaps orphaned children.
            if unsafe { prctl(36, 1, 0, 0, 0) } != 0 {
                return Err(format!(
                    "repository tools: adopt case descendants: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let child = command
            .spawn()
            .map_err(|error| format!("repository tools: spawn child: {error}"))?;
        let pid = i32::try_from(child.id()).map_err(|error| error.to_string())?;
        let owner = Owner { leader, timed };
        if interrupted() {
            cancel(pid, &owner);
        }
        owners.insert(pid, owner);
        Ok(child)
    }

    fn finish(pid: i32, status: std::process::ExitStatus) -> u8 {
        let mut owners = owners().lock().unwrap();
        let owner = owners.remove(&pid).expect("registered child");
        if owner.leader {
            unsafe {
                kill(-pid, 9);
            }
            #[cfg(target_os = "linux")]
            if owner.timed {
                // The direct child has been reaped; drain its adopted group descendants.
                while unsafe { waitpid(-pid, std::ptr::null_mut(), 0) } > 0 {}
            }
        }
        let interruption = INTERRUPTED.load(Ordering::SeqCst);
        let code = if interruption != 0 {
            128 + interruption
        } else {
            status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
        };
        u8::try_from(code).unwrap_or(1)
    }

    pub(super) fn interrupted() -> bool {
        INTERRUPTED.load(Ordering::SeqCst) != 0
    }

    pub(super) fn run(command: &mut Command) -> Result<u8, String> {
        let mut child = spawn(command, false)?;
        let pid = child.id() as i32;
        let status = child
            .wait()
            .map_err(|error| format!("repository tools: wait child: {error}"))?;
        Ok(finish(pid, status))
    }

    pub(super) fn output(command: &mut Command) -> Result<Output, String> {
        captured(command, None).map(|(output, _)| output)
    }

    pub(super) fn case_output(
        command: &mut Command,
        deadline: Duration,
    ) -> Result<(Output, bool), String> {
        captured(command, Some(deadline))
    }

    fn captured(
        command: &mut Command,
        deadline: Option<Duration>,
    ) -> Result<(Output, bool), String> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = spawn(command, deadline.is_some())?;
        let pid = child.id() as i32;
        let start = Instant::now();
        let (send, receive) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let _ = send.send(child.wait_with_output());
        });
        let (result, expired) = if let Some(deadline) = deadline {
            match receive.recv_timeout(deadline) {
                Ok(output) => (output, start.elapsed() >= deadline),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let owners = owners().lock().unwrap();
                    cancel(pid, owners.get(&pid).expect("registered case"));
                    drop(owners);
                    (receive.recv().map_err(|error| error.to_string())?, true)
                }
                Err(error) => return Err(error.to_string()),
            }
        } else {
            (receive.recv().map_err(|error| error.to_string())?, false)
        };
        waiter
            .join()
            .map_err(|_| "repository tools: child waiter panicked")?;
        let mut output =
            result.map_err(|error| format!("repository tools: wait child: {error}"))?;
        let code = finish(pid, output.status);
        output.status = std::process::ExitStatus::from_raw(i32::from(code) << 8);
        Ok((output, expired))
    }
}

#[cfg(not(unix))]
mod process {
    use std::process::Command;
    pub(super) fn interrupted() -> bool {
        false
    }
    pub(super) fn case_output(
        command: &mut Command,
        deadline: std::time::Duration,
    ) -> Result<(std::process::Output, bool), String> {
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = command.spawn().map_err(|error| error.to_string())?;
        let pid = child.id();
        let start = std::time::Instant::now();
        let (send, receive) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let _ = send.send(child.wait_with_output());
        });
        let (output, expired) = match receive.recv_timeout(deadline) {
            Ok(output) => (output, start.elapsed() >= deadline),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .status()
                    .map_err(|error| error.to_string())?;
                (receive.recv().map_err(|error| error.to_string())?, true)
            }
            Err(error) => return Err(error.to_string()),
        };
        waiter
            .join()
            .map_err(|_| "repository tools: child waiter panicked")?;
        Ok((output.map_err(|error| error.to_string())?, expired))
    }

    pub(super) fn run(command: &mut Command) -> Result<u8, String> {
        let status = command
            .status()
            .map_err(|error| format!("repository tools: spawn child: {error}"))?;
        Ok(status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1))
    }
    pub(super) fn output(command: &mut Command) -> Result<std::process::Output, String> {
        command
            .output()
            .map_err(|error| format!("repository tools: spawn child: {error}"))
    }
}
