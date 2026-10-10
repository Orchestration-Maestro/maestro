//! Controlled footer effects, fixture loading and child-process probes.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::atomic::{AtomicUsize, Ordering};

use maestro_app::presentation_data::footer_data_provider::{
    FooterDataProvider, FooterFileKind, FooterOperations,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// What a controlled path holds: text, or a flag for a directory (`true`) or
/// another kind of file (`false`).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    Text(String),
    Flag(bool),
}

/// How the controlled Git process ends.
#[derive(Clone)]
pub enum Git {
    Output(Option<String>),
    SpawnError,
}

/// Footer effects over an in-memory file system that records what is asked.
pub struct FakeOps {
    pub files: RefCell<BTreeMap<String, Entry>>,
    pub fail_stat: RefCell<Vec<String>>,
    pub fail_read: RefCell<Vec<String>>,
    pub git: RefCell<Git>,
    pub current: RefCell<Option<String>>,
    pub reads: RefCell<Vec<String>>,
    pub git_dirs: RefCell<Vec<String>>,
    pub current_dir_calls: Cell<usize>,
}

impl FakeOps {
    pub fn new(files: BTreeMap<String, Entry>) -> Rc<Self> {
        Rc::new(Self {
            files: RefCell::new(files),
            fail_stat: RefCell::default(),
            fail_read: RefCell::default(),
            git: RefCell::new(Git::Output(Some("main\n".to_owned()))),
            current: RefCell::default(),
            reads: RefCell::default(),
            git_dirs: RefCell::default(),
            current_dir_calls: Cell::new(0),
        })
    }

    pub fn fail(&self, stat: Vec<String>, read: Vec<String>) {
        *self.fail_stat.borrow_mut() = stat;
        *self.fail_read.borrow_mut() = read;
    }

    pub fn set(&self, path: &str, entry: Entry) {
        self.files.borrow_mut().insert(path.to_owned(), entry);
    }

    pub fn reads_of(&self, path: &str) -> usize {
        self.reads
            .borrow()
            .iter()
            .filter(|read| *read == path)
            .count()
    }
}

fn failure() -> io::Error {
    io::Error::other("controlled failure")
}

impl FooterOperations for FakeOps {
    fn exists(&self, path: &str) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn stat_kind(&self, path: &str) -> io::Result<FooterFileKind> {
        if self
            .fail_stat
            .borrow()
            .iter()
            .any(|failing| failing == path)
        {
            return Err(failure());
        }
        match self.files.borrow().get(path) {
            Some(Entry::Text(_)) => Ok(FooterFileKind::File),
            Some(Entry::Flag(true)) => Ok(FooterFileKind::Directory),
            Some(Entry::Flag(false)) => Ok(FooterFileKind::Other),
            None => Err(failure()),
        }
    }

    fn read_text(&self, path: &str) -> io::Result<String> {
        self.reads.borrow_mut().push(path.to_owned());
        match self.files.borrow().get(path) {
            Some(Entry::Text(text))
                if !self
                    .fail_read
                    .borrow()
                    .iter()
                    .any(|failing| failing == path) =>
            {
                Ok(text.clone())
            }
            _ => Err(failure()),
        }
    }

    fn current_dir(&self) -> io::Result<String> {
        self.current_dir_calls.set(self.current_dir_calls.get() + 1);
        self.current.borrow().clone().ok_or_else(failure)
    }

    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }

    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        self.git_dirs.borrow_mut().push(repo_dir.to_owned());
        match &*self.git.borrow() {
            Git::Output(output) => Ok(output.clone()),
            Git::SpawnError => Err(failure()),
        }
    }
}

/// A provider over `ops` for `cwd`.
pub fn provider(ops: &Rc<FakeOps>, cwd: &str) -> FooterDataProvider {
    FooterDataProvider::new(cwd.to_owned(), Rc::clone(ops) as Rc<dyn FooterOperations>)
}

/// The committed fixture cases of one test, in file order.
pub fn load<C: DeserializeOwned>(test: &str) -> Vec<C> {
    let all: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/footer_metadata.json"))
            .expect("fixture file parses");
    all.into_iter()
        .filter(|case| case["test"] == test)
        .inspect(|case| {
            let members = case.as_object().expect("fixture case is an object");
            assert!(
                members
                    .keys()
                    .all(|key| ["test", "input", "expected"].contains(&key.as_str()))
            );
        })
        .map(|case| serde_json::from_value(case).expect("fixture case matches its record"))
        .collect()
}

/// The files of a regular repository whose HEAD holds `head`.
pub fn plain(head: Entry) -> BTreeMap<String, Entry> {
    BTreeMap::from([
        ("/repo/.git".to_owned(), Entry::Flag(true)),
        ("/repo/.git/HEAD".to_owned(), head),
    ])
}

#[cfg(unix)]
/// A directory removed when dropped, on a best-effort basis.
pub struct Scratch(pub PathBuf);

#[cfg(unix)]
impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-footer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("scratch directory is new");
        Self(path)
    }

    pub fn join(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
/// Run `git` with fixed identity and no user configuration.
pub fn git(dir: &str, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git starts");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(unix)]
/// Run this test binary again for the single test `name`, with `envs`, and
/// return the `PROBE ` lines the child printed.
pub fn probe(name: &str, current_dir: &Path, envs: &[(&str, &str)]) -> Vec<String> {
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .current_dir(current_dir)
        .envs(envs.iter().copied())
        .output()
        .expect("child starts");
    assert!(output.status.success(), "child probe failed");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("PROBE ").map(str::to_owned))
        .collect()
}

#[cfg(unix)]
/// Copy `source` to `target` as an executable through a child process.
pub fn install_executable(source: &Path, target: &Path) {
    let status = Command::new("install")
        .args(["-m", "0700"])
        .arg(source)
        .arg(target)
        .status()
        .expect("install starts");
    assert!(status.success(), "install failed");
}
