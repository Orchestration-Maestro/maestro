//! Process-lifetime lease identities permit exit cleanup without taking a mutex.
//! Entries are published after acquisition, matching the synchronous lease registry.
//! A cross-thread exit between mkdir and publication can miss the new directory;
//! the single-threaded synchronous execution model has no such interleaving.
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex, Once,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    },
};

struct Entry {
    path: PathBuf,
    held: AtomicBool,
    next: Option<&'static Entry>,
}
static HEAD: AtomicPtr<Entry> = AtomicPtr::new(std::ptr::null_mut());
static REGISTER: Mutex<()> = Mutex::new(());
static INIT: Once = Once::new();
fn head() -> Option<&'static Entry> {
    // SAFETY: published entries are immutable except for atomics and never freed.
    unsafe { HEAD.load(Ordering::Acquire).as_ref() }
}
extern "C" fn cleanup() {
    let mut current = head();
    while let Some(entry) = current {
        if entry.held.load(Ordering::Acquire) {
            let _ = std::fs::remove_dir(&entry.path);
        }
        current = entry.next;
    }
}
fn register(path: &Path) -> &'static Entry {
    let _guard = REGISTER.lock().unwrap();
    let mut current = head();
    while let Some(entry) = current {
        if entry.path == path {
            return entry;
        }
        current = entry.next;
    }
    let entry = Box::leak(Box::new(Entry {
        path: path.to_owned(),
        held: AtomicBool::new(false),
        next: head(),
    }));
    HEAD.store(entry, Ordering::Release);
    entry
}
pub(in super::super) struct Section {
    entry: Option<&'static Entry>,
}
impl Section {
    pub(in super::super) fn enter() -> Self {
        STATE.fetch_add(SECTION, Ordering::SeqCst);
        INIT.call_once(|| {
            // SAFETY: cleanup has C ABI, remains valid for process lifetime and never unwinds.
            unsafe {
                libc::atexit(cleanup);
            }
            install();
        });
        Self { entry: None }
    }
    pub(in super::super) fn unhold(&self) {
        if let Some(entry) = self.entry {
            entry.held.store(false, Ordering::Release);
        }
    }
    pub(in super::super) fn hold(&mut self, path: &Path) {
        let entry = register(path);
        entry.held.store(true, Ordering::Release);
        self.entry = Some(entry);
    }
}
impl Drop for Section {
    fn drop(&mut self) {
        self.unhold();
        let previous = STATE.fetch_sub(SECTION, Ordering::SeqCst);
        if previous / SECTION == 1 && !previous.is_multiple_of(SECTION) {
            terminate(previous % SECTION);
        }
    }
}

// Low byte stores the first pending signal; upper bits count active sections.
const SECTION: usize = 256;
static STATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn defer(signal: usize) -> bool {
    let mut state = STATE.load(Ordering::SeqCst);
    loop {
        if state < SECTION {
            return false;
        }
        let next = if state.is_multiple_of(SECTION) {
            state | signal
        } else {
            state
        };
        match STATE.compare_exchange(state, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return true,
            Err(actual) => state = actual,
        }
    }
}
#[cfg(unix)]
extern "C" fn handler(signal: libc::c_int) {
    if !defer(signal as usize) {
        terminate(signal as usize);
    }
}
#[cfg(unix)]
fn terminate(signal: usize) {
    // SAFETY: a zeroed sigaction with SIG_DFL and an empty mask is valid; sigaction,
    // sigemptyset, getpid and kill are async-signal-safe and use no shared locks.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(signal as libc::c_int, &action, std::ptr::null_mut());
        libc::kill(libc::getpid(), signal as libc::c_int);
    }
}
#[cfg(unix)]
fn install() {
    let signals = [
        libc::SIGABRT,
        libc::SIGALRM,
        libc::SIGHUP,
        libc::SIGINT,
        libc::SIGTERM,
        libc::SIGVTALRM,
        libc::SIGXCPU,
        libc::SIGXFSZ,
        libc::SIGUSR2,
        libc::SIGTRAP,
        libc::SIGSYS,
        libc::SIGQUIT,
        #[cfg(target_os = "linux")]
        libc::SIGIO,
        #[cfg(target_os = "linux")]
        libc::SIGPOLL,
        #[cfg(target_os = "linux")]
        libc::SIGPWR,
        #[cfg(target_os = "linux")]
        libc::SIGSTKFLT,
    ];
    for signal in signals {
        // SAFETY: both sigaction buffers and masks are initialized, pointers remain
        // valid for the call, and the handler has the required ABI and lifetime.
        unsafe {
            let mut prior: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(signal, std::ptr::null(), &mut prior) != 0
                || prior.sa_sigaction != libc::SIG_DFL
            {
                continue;
            }
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handler as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
}
#[cfg(windows)]
unsafe extern "system" fn console_handler(event: u32) -> windows_sys::core::BOOL {
    use windows_sys::Win32::System::Console::{CTRL_C_EVENT, CTRL_CLOSE_EVENT};
    let signal = match event {
        CTRL_C_EVENT => 2,
        CTRL_CLOSE_EVENT => 1,
        _ => return 0,
    };
    if !defer(signal) {
        terminate(signal);
    } else if event == CTRL_CLOSE_EVENT {
        // SAFETY: parking this control thread lets active sections release before
        // termination; returning from a close callback would terminate immediately.
        unsafe {
            windows_sys::Win32::System::Threading::Sleep(u32::MAX);
        }
    }
    1
}
#[cfg(windows)]
fn install() {
    // SAFETY: the callback has the console-handler ABI and process lifetime.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_handler), 1);
    }
}
#[cfg(windows)]
fn terminate(_: usize) {
    cleanup();
    // SAFETY: GetCurrentProcess returns the current process pseudo-handle;
    // TerminateProcess matches signal-based forced termination with exit code 1.
    unsafe {
        windows_sys::Win32::System::Threading::TerminateProcess(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            1,
        );
    }
}
