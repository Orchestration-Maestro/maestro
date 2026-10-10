//! Controlled footer effects, fixture loading and child-process probes.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io;
use std::rc::Rc;

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

/// A fixture path spelled for the target: on Windows, slash-rooted paths gain
/// `C:` and separators become backslashes; relative paths stay relative.
pub fn target(path: &str) -> String {
    if cfg!(windows) {
        let separated = path.replace('/', "\\");
        if path.starts_with('/') {
            format!("C:{separated}")
        } else {
            separated
        }
    } else {
        path.to_owned()
    }
}

/// The fixture spelling of a path the provider produced for the target.
fn portable(path: &str) -> String {
    if cfg!(windows) {
        path.strip_prefix("C:").unwrap_or(path).replace('\\', "/")
    } else {
        path.to_owned()
    }
}

impl FakeOps {
    pub fn new(files: BTreeMap<String, Entry>) -> Rc<Self> {
        Rc::new(Self {
            files: RefCell::new(
                files
                    .into_iter()
                    .map(|(path, entry)| (target(&path), entry))
                    .collect(),
            ),
            fail_stat: RefCell::default(),
            fail_read: RefCell::default(),
            git: RefCell::new(Git::Output(Some("main\n".to_owned()))),
            current: RefCell::default(),
            reads: RefCell::default(),
            git_dirs: RefCell::default(),
            current_dir_calls: Cell::new(0),
        })
    }

    pub fn fail(&self, stat: &[String], read: &[String]) {
        *self.fail_stat.borrow_mut() = stat.iter().map(|path| target(path)).collect();
        *self.fail_read.borrow_mut() = read.iter().map(|path| target(path)).collect();
    }

    pub fn set(&self, path: &str, entry: Entry) {
        self.files.borrow_mut().insert(target(path), entry);
    }

    pub fn reads_of(&self, path: &str) -> usize {
        let path = target(path);
        self.reads
            .borrow()
            .iter()
            .filter(|read| **read == path)
            .count()
    }
}

fn failure() -> io::Error {
    io::Error::other("controlled failure")
}

impl FooterOperations for FakeOps {
    fn symbolic_ref_async(
        &self,
        repo: &str,
        callback: Box<dyn FnOnce(io::Result<Option<String>>)>,
    ) {
        callback(self.symbolic_ref_sync(repo));
    }
    fn watch_file(
        &self,
        _: &str,
        _: std::time::Duration,
        _: Rc<
            dyn Fn(
                maestro_app::presentation_data::footer_data_provider::FooterFileStats,
                maestro_app::presentation_data::footer_data_provider::FooterFileStats,
            ),
        >,
    ) -> Box<dyn maestro_watch::fs_watch::FsWatcher> {
        unreachable!("controlled metadata adapter rejects watch registration")
    }
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
        self.current
            .borrow()
            .as_deref()
            .map(target)
            .ok_or_else(failure)
    }

    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }

    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        self.git_dirs.borrow_mut().push(portable(repo_dir));
        match &*self.git.borrow() {
            Git::Output(output) => Ok(output.clone()),
            Git::SpawnError => Err(failure()),
        }
    }
}

/// A provider over `ops` for `cwd`.
pub fn provider(ops: &Rc<FakeOps>, cwd: &str) -> FooterDataProvider {
    FooterDataProvider::new(target(cwd), Rc::clone(ops) as Rc<dyn FooterOperations>)
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
mod native;
#[cfg(unix)]
pub use native::{Scratch, git, install_executable, probe};

impl maestro_watch::fs_watch::WatchOperations for FakeOps {
    fn watch(
        &self,
        _: &str,
        _: Rc<dyn Fn(Option<String>)>,
        _: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn maestro_watch::fs_watch::FsWatcher>> {
        Err(failure())
    }
    fn schedule(
        &self,
        _: std::time::Duration,
        _: Box<dyn FnOnce()>,
    ) -> Box<dyn maestro_watch::fs_watch::WatchTimer> {
        Box::new(Cancel)
    }
}
struct Cancel;
impl maestro_watch::fs_watch::WatchTimer for Cancel {
    fn cancel(&mut self) {}
}
