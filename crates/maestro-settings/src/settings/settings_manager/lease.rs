mod termination;
use super::{Error, value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) struct LeaseError {
    pub(super) code: &'static str,
    message: String,
    file: PathBuf,
    cause: Option<Error>,
}
impl std::fmt::Debug for LeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeaseError")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("file", &self.file)
            .field("cause", &self.cause)
            .finish()
    }
}
impl std::fmt::Display for LeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for LeaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|e| e.as_ref() as _)
    }
}
fn error(code: &'static str, message: &str, file: &Path, cause: Option<Error>) -> Error {
    Box::new(LeaseError {
        code,
        message: message.into(),
        file: file.to_owned(),
        cause,
    })
}
fn is_kind(error: &Error, kind: std::io::ErrorKind) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == kind)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Precision {
    Milliseconds,
    Seconds,
}
struct Heartbeat {
    mtime: i64,
    precision: Precision,
    last_update: i64,
    next: Option<i64>,
    released: bool,
}
type Registry = Arc<Mutex<BTreeMap<PathBuf, Arc<Mutex<Heartbeat>>>>>;
trait Runtime: Send + Sync {
    fn now(&self) -> i64;
    fn mkdir(&self, path: &Path) -> Result<(), Error>;
    fn stat(&self, path: &Path) -> Result<i64, Error>;
    fn utimes(&self, path: &Path, time: i64) -> Result<(), Error>;
    fn remove(&self, path: &Path) -> Result<(), Error>;
    fn delay(&self, ms: u64);
    fn precision(&self) -> Option<Precision>;
    fn cache_precision(&self, precision: Precision);
    fn registry(&self) -> Registry;
}
struct Native;
static PRECISION: OnceLock<Precision> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();
impl Runtime for Native {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
    fn mkdir(&self, path: &Path) -> Result<(), Error> {
        Ok(std::fs::create_dir(path)?)
    }
    fn stat(&self, path: &Path) -> Result<i64, Error> {
        let modified = std::fs::metadata(path)?.modified()?;
        Ok(match modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis() as i64,
            Err(error) => -(error.duration().as_millis() as i64),
        })
    }
    fn utimes(&self, path: &Path, time: i64) -> Result<(), Error> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.access_mode(0x100).custom_flags(0x02000000);
        }
        let stamp = UNIX_EPOCH + Duration::from_millis(time as u64);
        Ok(options.open(path)?.set_times(
            std::fs::FileTimes::new()
                .set_accessed(stamp)
                .set_modified(stamp),
        )?)
    }
    fn remove(&self, path: &Path) -> Result<(), Error> {
        Ok(std::fs::remove_dir(path)?)
    }
    fn delay(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn precision(&self) -> Option<Precision> {
        PRECISION.get().copied()
    }
    fn cache_precision(&self, precision: Precision) {
        let _ = PRECISION.set(precision);
    }
    fn registry(&self) -> Registry {
        REGISTRY.get_or_init(Registry::default).clone()
    }
}
fn remove(runtime: &dyn Runtime, path: &Path) -> Result<(), Error> {
    match runtime.remove(path) {
        Err(e) if is_kind(&e, std::io::ErrorKind::NotFound) => Ok(()),
        result => result,
    }
}
fn probe(runtime: &dyn Runtime, path: &Path) -> Result<(i64, Precision), Error> {
    if let Some(precision) = runtime.precision() {
        return Ok((runtime.stat(path)?, precision));
    }
    let now = runtime.now();
    runtime.utimes(path, ((now + 999) / 1000) * 1000 + 5)?;
    let mtime = runtime.stat(path)?;
    let precision = if mtime % 1000 == 0 {
        Precision::Seconds
    } else {
        Precision::Milliseconds
    };
    runtime.cache_precision(precision);
    Ok((mtime, precision))
}
fn attempt(
    runtime: &dyn Runtime,
    file: &Path,
    path: &Path,
    stale: bool,
) -> Result<(i64, Precision), Error> {
    match runtime.mkdir(path) {
        Ok(()) => match probe(runtime, path) {
            Ok(v) => Ok(v),
            Err(e) => {
                let _ = runtime.remove(path);
                Err(e)
            }
        },
        Err(e) if is_kind(&e, std::io::ErrorKind::AlreadyExists) => {
            if !stale {
                return Err(error(
                    "ELOCKED",
                    "Lock file is already being held",
                    file,
                    None,
                ));
            }
            let mtime = match runtime.stat(path) {
                Ok(mtime) => mtime,
                Err(e) if is_kind(&e, std::io::ErrorKind::NotFound) => {
                    return attempt(runtime, file, path, false);
                }
                Err(e) => return Err(e),
            };
            if mtime >= runtime.now() - 10000 {
                return Err(error(
                    "ELOCKED",
                    "Lock file is already being held",
                    file,
                    None,
                ));
            }
            remove(runtime, path)?;
            attempt(runtime, file, path, false)
        }
        Err(e) => Err(e),
    }
}
pub(super) struct Lease {
    file: PathBuf,
    path: PathBuf,
    state: Arc<Mutex<Heartbeat>>,
    runtime: Arc<dyn Runtime>,
    section: Option<termination::Section>,
}
impl Lease {
    pub(super) fn acquire(file: &Path) -> Result<Self, Error> {
        let mut section = termination::Section::enter();
        let mut lease = Self::acquire_with(file, Arc::new(Native))?;
        section.hold(&lease.path);
        lease.section = Some(section);
        Ok(lease)
    }
    fn acquire_with(file: &Path, runtime: Arc<dyn Runtime>) -> Result<Self, Error> {
        Self::acquire_attempts(file, runtime, 10)
    }
    fn acquire_attempts(
        file: &Path,
        runtime: Arc<dyn Runtime>,
        attempts: usize,
    ) -> Result<Self, Error> {
        let file = if file.is_absolute() {
            value::join(&[file])
        } else {
            value::join(&[&std::env::current_dir()?, file])
        };
        let mut path = file.as_os_str().to_owned();
        path.push(".lock");
        let path = PathBuf::from(path);
        for index in 0..attempts {
            match attempt(runtime.as_ref(), &file, &path, true) {
                Ok((mtime, precision)) => {
                    let now = runtime.now();
                    let state = Arc::new(Mutex::new(Heartbeat {
                        mtime,
                        precision,
                        last_update: now,
                        next: Some(now + 5000),
                        released: false,
                    }));
                    runtime
                        .registry()
                        .lock()
                        .unwrap()
                        .insert(file.clone(), state.clone());
                    return Ok(Self {
                        file,
                        path,
                        state,
                        runtime,
                        section: None,
                    });
                }
                Err(e) => {
                    if e.downcast_ref::<LeaseError>()
                        .is_none_or(|e| e.code != "ELOCKED")
                        || index + 1 == attempts
                    {
                        return Err(e);
                    }
                    runtime.delay(20);
                }
            }
        }
        Err(std::io::Error::other("Failed to acquire settings lock").into())
    }
    pub(super) fn release(&mut self) -> Result<(), Error> {
        if self.state.lock().unwrap().released {
            return Err(error(
                "ERELEASED",
                "Lock is already released",
                &self.file,
                None,
            ));
        }
        let state = self
            .runtime
            .registry()
            .lock()
            .unwrap()
            .remove(&self.file)
            .ok_or_else(|| {
                error(
                    "ENOTACQUIRED",
                    "Lock is not acquired/owned by you",
                    &self.file,
                    None,
                )
            })?;
        {
            let mut state = state.lock().unwrap();
            state.released = true;
            state.next = None;
        }
        if let Some(section) = &self.section {
            section.unhold();
        }
        let result = remove(self.runtime.as_ref(), &self.path);
        self.section.take();
        result
    }
    // Synchronous storage releases the lease in the same turn; timer work cannot
    // execute inside its callback. The state transition remains independently testable.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Synchronous callbacks do not yield to heartbeat timers"
        )
    )]
    fn heartbeat(&mut self) -> Result<(), Error> {
        let now = self.runtime.now();
        let (mtime, precision, last_update) = {
            let mut state = self.state.lock().unwrap();
            if state.released || state.next.is_none_or(|next| next > now) {
                return Ok(());
            }
            state.next = None;
            (state.mtime, state.precision, state.last_update)
        };
        let result = match self.runtime.stat(&self.path) {
            Ok(actual) if actual != mtime => Err(error(
                "ECOMPROMISED",
                "Unable to update lock within the stale threshold",
                &self.file,
                None,
            )),
            Ok(_) => {
                let mtime = if precision == Precision::Seconds {
                    ((now + 999) / 1000) * 1000
                } else {
                    now
                };
                self.runtime.utimes(&self.path, mtime).map(|()| mtime)
            }
            Err(e) => Err(e),
        };
        let mut state = self.state.lock().unwrap();
        if state.released {
            return Ok(());
        }
        match result {
            Ok(mtime) => {
                state.mtime = mtime;
                state.last_update = self.runtime.now();
                state.next = Some(state.last_update + 5000);
                Ok(())
            }
            Err(e) => {
                if is_kind(&e, std::io::ErrorKind::NotFound)
                    || last_update + 10000 < self.runtime.now()
                    || e.downcast_ref::<LeaseError>()
                        .is_some_and(|e| e.code == "ECOMPROMISED")
                {
                    state.released = true;
                    state.next = None;
                    drop(state);
                    let registry = self.runtime.registry();
                    let mut registry = registry.lock().unwrap();
                    if registry
                        .get(&self.file)
                        .is_some_and(|current| Arc::ptr_eq(current, &self.state))
                    {
                        registry.remove(&self.file);
                    }
                    drop(registry);
                    if e.downcast_ref::<LeaseError>()
                        .is_some_and(|e| e.code == "ECOMPROMISED")
                    {
                        Err(e)
                    } else {
                        Err(error("ECOMPROMISED", &e.to_string(), &self.file, Some(e)))
                    }
                } else {
                    state.next = Some(self.runtime.now() + 1000);
                    Ok(())
                }
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.state.lock().unwrap().released {
            let _ = self.release();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    };
    struct Fake {
        now: AtomicI64,
        mtime: Mutex<Option<i64>>,
        precision: Mutex<Option<Precision>>,
        seconds: bool,
        events: Mutex<Vec<String>>,
        failures: Mutex<VecDeque<(&'static str, std::io::ErrorKind)>>,
        vanish: AtomicBool,
        release_in_gaps: AtomicUsize,
        registry: Registry,
    }
    impl Fake {
        fn new(now: i64, mtime: Option<i64>, seconds: bool) -> Arc<Self> {
            Arc::new(Self {
                now: AtomicI64::new(now),
                mtime: Mutex::new(mtime),
                precision: Mutex::new(None),
                seconds,
                events: Mutex::new(Vec::new()),
                failures: Mutex::new(VecDeque::new()),
                vanish: AtomicBool::new(false),
                release_in_gaps: AtomicUsize::new(0),
                registry: Registry::default(),
            })
        }
        fn event(&self, name: &'static str) -> Result<(), Error> {
            self.events.lock().unwrap().push(name.into());
            let mut failures = self.failures.lock().unwrap();
            if failures.front().is_some_and(|(n, _)| *n == name) {
                let (_, kind) = failures.pop_front().unwrap();
                return Err(std::io::Error::new(kind, format!("sentinel {name}")).into());
            }
            Ok(())
        }
        fn fail(&self, name: &'static str, kind: std::io::ErrorKind) {
            self.failures.lock().unwrap().push_back((name, kind));
        }
        fn count(&self, name: &str) -> usize {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.as_str() == name)
                .count()
        }
    }
    impl Runtime for Fake {
        fn now(&self) -> i64 {
            self.now.load(Ordering::SeqCst)
        }
        fn mkdir(&self, _: &Path) -> Result<(), Error> {
            self.event("mkdir")?;
            let mut m = self.mtime.lock().unwrap();
            if m.is_some() {
                return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
            }
            *m = Some(self.now());
            Ok(())
        }
        fn stat(&self, _: &Path) -> Result<i64, Error> {
            self.event("stat")?;
            if self.vanish.swap(false, Ordering::SeqCst) {
                *self.mtime.lock().unwrap() = None;
            }
            self.mtime
                .lock()
                .unwrap()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound).into())
        }
        fn utimes(&self, _: &Path, time: i64) -> Result<(), Error> {
            self.event("utimes")?;
            let mut m = self.mtime.lock().unwrap();
            if m.is_none() {
                return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
            }
            *m = Some(if self.seconds {
                time / 1000 * 1000
            } else {
                time
            });
            Ok(())
        }
        fn remove(&self, _: &Path) -> Result<(), Error> {
            self.event("remove")?;
            self.mtime
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
            Ok(())
        }
        fn delay(&self, ms: u64) {
            self.events.lock().unwrap().push(format!("delay:{ms}"));
            if self
                .release_in_gaps
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                == Ok(1)
            {
                *self.mtime.lock().unwrap() = None;
            }
        }
        fn precision(&self) -> Option<Precision> {
            *self.precision.lock().unwrap()
        }
        fn cache_precision(&self, p: Precision) {
            *self.precision.lock().unwrap() = Some(p);
        }
        fn registry(&self) -> Registry {
            self.registry.clone()
        }
    }
    fn file() -> &'static Path {
        Path::new("/synthetic/preferences.json")
    }

    #[test]
    fn contention_attempts_and_gaps_are_exact() {
        for success in 1..=10 {
            let f = Fake::new(20000, None, false);
            for _ in 1..success {
                f.fail("mkdir", std::io::ErrorKind::AlreadyExists);
                f.fail("stat", std::io::ErrorKind::PermissionDenied);
            } // Noncontention stat errors must not retry.
            if success > 1 {
                let e = Lease::acquire_with(file(), f.clone()).err().unwrap();
                assert_eq!(e.to_string(), "sentinel stat");
                assert_eq!(f.count("mkdir"), 1);
            } else {
                let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
                l.release().unwrap();
            }
        }
        let f = Fake::new(20000, Some(20000), false);
        let e = Lease::acquire_with(file(), f.clone()).err().unwrap();
        assert_eq!(e.downcast_ref::<LeaseError>().unwrap().code, "ELOCKED");
        assert_eq!(f.count("mkdir"), 10);
        assert_eq!(f.count("delay:20"), 9);
        for success in 1..=10 {
            let f = Fake::new(20000, if success == 1 { None } else { Some(20000) }, false);
            f.release_in_gaps.store(success - 1, Ordering::SeqCst);
            let mut lease = Lease::acquire_with(file(), f.clone()).unwrap();
            assert_eq!(f.count("mkdir"), success);
            assert_eq!(f.count("delay:20"), success - 1);
            lease.release().unwrap();
        }
        let f = Fake::new(20000, None, false);
        f.fail("mkdir", std::io::ErrorKind::PermissionDenied);
        assert!(Lease::acquire_with(file(), f.clone()).is_err());
        assert_eq!(f.count("mkdir"), 1);
        assert_eq!(f.count("delay:20"), 0);
    }
    #[test]
    fn stale_boundary_precision_and_disappearance() {
        let f = Fake::new(20000, Some(10000), false);
        assert!(Lease::acquire_with(file(), f.clone()).is_err());
        assert_eq!(f.count("remove"), 0);
        let f = Fake::new(20000, Some(9999), false);
        let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
        assert_eq!(f.count("remove"), 1);
        assert_eq!(f.count("mkdir"), 2);
        l.release().unwrap();
        for seconds in [false, true] {
            let f = Fake::new(20001, None, seconds);
            let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
            assert_eq!(
                l.state.lock().unwrap().mtime,
                if seconds { 21000 } else { 21005 }
            );
            assert_eq!(f.count("utimes"), 1);
            l.release().unwrap();
            let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
            assert_eq!(f.count("utimes"), 1);
            l.release().unwrap();
        }
        let f = Fake::new(20000, Some(10000), false);
        f.vanish.store(true, Ordering::SeqCst);
        let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
        assert_eq!(f.count("mkdir"), 2);
        l.release().unwrap();
        for method in ["utimes", "stat"] {
            let f = Fake::new(20000, None, false);
            f.fail(method, std::io::ErrorKind::PermissionDenied);
            assert!(Lease::acquire_with(file(), f.clone()).is_err());
            assert_eq!(f.count("remove"), 1);
            assert!(f.mtime.lock().unwrap().is_none());
        }
    }
    #[test]
    fn heartbeat_renewal_retry_and_compromise() {
        let f = Fake::new(20001, None, false);
        let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
        f.now.store(25000, Ordering::SeqCst);
        l.heartbeat().unwrap();
        assert_eq!(f.count("utimes"), 1);
        f.now.store(25001, Ordering::SeqCst);
        l.heartbeat().unwrap();
        assert_eq!(f.count("utimes"), 2);
        assert_eq!(l.state.lock().unwrap().mtime, 25001);
        f.now.store(30001, Ordering::SeqCst);
        f.fail("stat", std::io::ErrorKind::PermissionDenied);
        l.heartbeat().unwrap();
        assert_eq!(l.state.lock().unwrap().next, Some(31001));
        f.now.store(31001, Ordering::SeqCst);
        l.heartbeat().unwrap();
        assert_eq!(l.state.lock().unwrap().next, Some(36001));
        f.now.store(36001, Ordering::SeqCst);
        f.fail("utimes", std::io::ErrorKind::PermissionDenied);
        l.heartbeat().unwrap();
        assert_eq!(l.state.lock().unwrap().next, Some(37001));
        f.now.store(37001, Ordering::SeqCst);
        l.heartbeat().unwrap();
        l.release().unwrap();
        let count = f.count("stat");
        f.now.store(50000, Ordering::SeqCst);
        l.heartbeat().unwrap();
        assert_eq!(f.count("stat"), count);
        for (method, kind, over) in [
            ("stat", std::io::ErrorKind::NotFound, false),
            ("stat", std::io::ErrorKind::PermissionDenied, true),
            ("utimes", std::io::ErrorKind::NotFound, false),
            ("utimes", std::io::ErrorKind::PermissionDenied, true),
        ] {
            let f = Fake::new(20000, None, false);
            let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
            f.now
                .store(if over { 30001 } else { 25000 }, Ordering::SeqCst);
            f.fail(method, kind);
            let e = l.heartbeat().unwrap_err();
            assert_eq!(e.downcast_ref::<LeaseError>().unwrap().code, "ECOMPROMISED");
            assert!(std::error::Error::source(e.as_ref()).is_some());
            assert_eq!(
                l.release().unwrap_err().to_string(),
                "Lock is already released"
            );
            assert!(f.registry.lock().unwrap().is_empty());
        }
        let f = Fake::new(20000, None, false);
        let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
        *f.mtime.lock().unwrap() = Some(25000);
        f.now.store(25000, Ordering::SeqCst);
        let e = l.heartbeat().unwrap_err();
        assert_eq!(
            e.to_string(),
            "Unable to update lock within the stale threshold"
        );
    }
    #[test]
    fn release_replacement_and_error_messages() {
        let f = Fake::new(20000, None, false);
        let mut first = Lease::acquire_with(file(), f.clone()).unwrap();
        f.now.store(31006, Ordering::SeqCst);
        let mut second = Lease::acquire_with(file(), f.clone()).unwrap();
        first.release().unwrap();
        assert_eq!(
            second.release().unwrap_err().to_string(),
            "Lock is already released"
        );
        assert_eq!(
            first.release().unwrap_err().to_string(),
            "Lock is not acquired/owned by you"
        );
        let f = Fake::new(20000, None, false);
        let mut l = Lease::acquire_with(file(), f.clone()).unwrap();
        f.fail("remove", std::io::ErrorKind::PermissionDenied);
        assert_eq!(l.release().unwrap_err().to_string(), "sentinel remove");
        assert_eq!(
            l.release()
                .unwrap_err()
                .downcast_ref::<LeaseError>()
                .unwrap()
                .code,
            "ERELEASED"
        );
        let f = Fake::new(20000, None, false);
        let e = Lease::acquire_attempts(file(), f, 0).err().unwrap();
        assert_eq!(e.to_string(), "Failed to acquire settings lock");
    }
}
