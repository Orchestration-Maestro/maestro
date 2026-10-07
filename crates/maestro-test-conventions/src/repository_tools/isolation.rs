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
                "mi-{:x}-{:x}",
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
        if let Err(error) = fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
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
    for name in ["MAESTRO_CASE_SCRATCH"] {
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

// Each invocation has one owner and one child tree. The runner trusts the owner
// to exit after its process-table-bounded sweep; there is no fallback timer.
fn owned(command: Command) -> Result<(Command, Scratch), String> {
    let mut owner = Command::new(std::env::current_exe().map_err(|error| error.to_string())?);
    owner
        .arg("isolation-owner")
        .arg(command.get_program())
        .args(command.get_args());
    if let Some(directory) = command.get_current_dir() {
        owner.current_dir(directory);
    }
    for (name, value) in command.get_envs() {
        if let Some(value) = value {
            owner.env(name, value);
        } else {
            owner.env_remove(name);
        }
    }
    let scratch = prepare(&mut owner)?;
    owner.env("MAESTRO_CASE_SCRATCH", &scratch.0);
    owner.env("MAESTRO_OWNER_SCRATCH", &scratch.0);
    Ok((owner, scratch))
}

pub(super) fn owner(args: &[std::ffi::OsString]) -> Result<u8, String> {
    let executable = args
        .first()
        .ok_or("repository tools: expected owner child")?;
    let scratch = Scratch(
        std::env::var_os("MAESTRO_OWNER_SCRATCH")
            .map(PathBuf::from)
            .ok_or("repository tools: missing owner scratch")?,
    );
    let mut command = Command::new(executable);
    command.args(&args[1..]).env_remove("MAESTRO_OWNER_SCRATCH");
    let result = process::owner(&mut command);
    drop(scratch);
    result
}

pub(super) fn finish() {
    process::sweep();
}

pub(super) fn run(command: Command) -> Result<u8, String> {
    let (mut command, _scratch) = owned(command)?;
    process::run(&mut command)
}

pub(super) fn output(command: Command) -> Result<std::process::Output, String> {
    let (mut command, _scratch) = owned(command)?;
    process::output(&mut command)
}

pub(super) fn interrupted() -> bool {
    process::interrupted()
}

pub(super) fn case_output(
    command: Command,
    deadline: std::time::Duration,
) -> Result<(std::process::Output, bool), String> {
    let (mut command, _scratch) = owned(command)?;
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
        group: bool,
    }

    unsafe extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
        fn kill(pid: i32, number: i32) -> i32;
        fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
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
        unsafe {
            kill(
                if owner.group { -pid } else { pid },
                if owner.group { 9 } else { 15 },
            );
        }
    }

    fn spawn(command: &mut Command, group: bool) -> Result<Child, String> {
        let mut owners = owners().lock().unwrap();
        if group {
            command.process_group(0);
        }
        #[cfg(target_os = "linux")]
        if unsafe { prctl(36, 1, 0, 0, 0) } != 0 {
            return Err(format!(
                "repository tools: adopt descendants: {}",
                std::io::Error::last_os_error()
            ));
        }
        let child = command
            .spawn()
            .map_err(|error| format!("repository tools: spawn child: {error}"))?;
        let pid = i32::try_from(child.id()).map_err(|error| error.to_string())?;
        let owner = Owner { group };
        if interrupted() {
            cancel(pid, &owner);
        }
        owners.insert(pid, owner);
        Ok(child)
    }

    pub(super) fn sweep() {
        #[cfg(target_os = "linux")]
        loop {
            // Include every thread's children: concurrent spawns belong to their thread.
            let mut children = Vec::new();
            if let Ok(tasks) = std::fs::read_dir("/proc/self/task") {
                for task in tasks.flatten() {
                    if let Ok(text) = std::fs::read_to_string(task.path().join("children")) {
                        children.extend(
                            text.split_whitespace()
                                .filter_map(|pid| pid.parse::<i32>().ok()),
                        );
                    }
                }
            }
            if children.is_empty() {
                break;
            }
            for pid in &children {
                unsafe {
                    kill(*pid, 9);
                }
            }
            for pid in children {
                unsafe {
                    waitpid(pid, std::ptr::null_mut(), 0);
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        unsafe {
            while waitpid(-1, std::ptr::null_mut(), 0) > 0 {}
        }
    }

    pub(super) fn owner(command: &mut Command) -> Result<u8, String> {
        let mut child = spawn(command, true)?;
        let pid = child.id() as i32;
        let result = child
            .wait()
            .map_err(|error| format!("repository tools: wait child: {error}"));
        let result = result.map(|status| finish(pid, status));
        sweep();
        result
    }

    fn finish(pid: i32, status: std::process::ExitStatus) -> u8 {
        let mut owners = owners().lock().unwrap();
        let owner = owners.remove(&pid).expect("registered child");
        if owner.group {
            unsafe {
                kill(-pid, 9);
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
        let child = spawn(command, false)?;
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

#[cfg(windows)]
mod process {
    use std::ffi::c_void;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Output, Stdio};
    use std::time::{Duration, Instant};

    // ABI from windows-sys 0.61.2: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    // JOBOBJECT_EXTENDED_LIMIT_INFORMATION, IO_COUNTERS, THREADENTRY32.
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimits {
        process_time: i64,
        job_time: i64,
        flags: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }
    #[repr(C)]
    #[derive(Default)]
    struct ThreadEntry {
        size: u32,
        usage: u32,
        thread: u32,
        process: u32,
        base_priority: i32,
        delta_priority: i32,
        flags: u32,
    }
    // Signatures and constants from windows-sys 0.61.2, Win32 System:
    // JobObjects::{CreateJobObjectW, SetInformationJobObject,
    // AssignProcessToJobObject, TerminateJobObject}, Threading::{OpenThread,
    // ResumeThread}, Diagnostics::ToolHelp::{CreateToolhelp32Snapshot,
    // Thread32First, Thread32Next}. SECURITY_ATTRIBUTES is always null.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
        fn SetInformationJobObject(
            job: *mut c_void,
            class: i32,
            info: *const c_void,
            size: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
        fn TerminateJobObject(job: *mut c_void, code: u32) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
        fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
        fn OpenThread(access: u32, inherit: i32, tid: u32) -> *mut c_void;
        fn ResumeThread(thread: *mut c_void) -> u32;
    }
    const KILL_ON_CLOSE: u32 = 8192; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    const EXTENDED_LIMITS: i32 = 9; // JobObjectExtendedLimitInformation
    const SUSPENDED: u32 = 4; // CREATE_SUSPENDED
    const SNAP_THREADS: u32 = 4; // TH32CS_SNAPTHREAD
    const RESUME_ACCESS: u32 = 2; // THREAD_SUSPEND_RESUME

    fn error() -> String {
        std::io::Error::last_os_error().to_string()
    }

    fn spawn(command: &mut Command) -> Result<(Child, OwnedHandle), String> {
        // SAFETY: Null attributes/name request a private, non-inheritable job.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(error());
        }
        // SAFETY: The successful creation returns an owned kernel handle.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let limits = ExtendedLimits {
            basic: BasicLimits {
                flags: KILL_ON_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: The buffer has the exact extended-limit layout and length.
        if unsafe {
            SetInformationJobObject(
                raw,
                EXTENDED_LIMITS,
                (&limits as *const ExtendedLimits).cast(),
                std::mem::size_of::<ExtendedLimits>() as u32,
            )
        } == 0
        {
            return Err(error());
        }
        let mut child = command
            .creation_flags(SUSPENDED)
            .spawn()
            .map_err(|error| error.to_string())?;
        // SAFETY: Both handles are live; suspension prevents unowned descendants.
        if unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0 {
            let error = error();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        if let Err(error) = resume(child.id()) {
            drop(job);
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, job))
    }

    fn resume(pid: u32) -> Result<(), String> {
        // SAFETY: Flags request a read-only snapshot of the system's threads.
        let raw = unsafe { CreateToolhelp32Snapshot(SNAP_THREADS, 0) };
        if raw == -1isize as *mut c_void {
            return Err(error());
        }
        // SAFETY: A successful snapshot returns a uniquely owned handle.
        let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut entry = ThreadEntry {
            size: std::mem::size_of::<ThreadEntry>() as u32,
            ..Default::default()
        };
        // SAFETY: The snapshot is live and entry is a writable THREADENTRY32.
        let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
        while found != 0 {
            if entry.process == pid {
                // SAFETY: The ID came from the snapshot; only resume access is requested.
                let raw = unsafe { OpenThread(RESUME_ACCESS, 0, entry.thread) };
                if raw.is_null() {
                    return Err(error());
                }
                // SAFETY: OpenThread returned a uniquely owned handle.
                let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
                // SAFETY: This is the suspended child's primary thread.
                if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                    return Err(error());
                }
                return Ok(());
            }
            // SAFETY: The snapshot and writable entry remain valid.
            found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
        }
        Err("repository tools: suspended child thread missing".into())
    }

    pub(super) fn interrupted() -> bool {
        false
    }
    // Each lexical job is closed before returning, including on an error.
    pub(super) fn sweep() {}
    pub(super) fn owner(command: &mut Command) -> Result<u8, String> {
        run(command)
    }
    pub(super) fn run(command: &mut Command) -> Result<u8, String> {
        let (mut child, job) = spawn(command)?;
        let status = child.wait().map_err(|error| error.to_string());
        drop(job);
        status.map(super::super::status_code)
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
        let (child, job) = spawn(command)?;
        let start = Instant::now();
        let (send, receive) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let _ = send.send(child.wait_with_output());
        });
        let (output, expired) = if let Some(deadline) = deadline {
            match receive.recv_timeout(deadline) {
                Ok(output) => (output, start.elapsed() >= deadline),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // SAFETY: The live job contains only this invocation's subtree.
                    if unsafe { TerminateJobObject(job.as_raw_handle(), 1) } == 0 {
                        return Err(error());
                    }
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
        drop(job);
        Ok((output.map_err(|error| error.to_string())?, expired))
    }
}
