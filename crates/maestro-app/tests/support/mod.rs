//! Controlled footer effects, fixture loading and child-process probes.
#![cfg(not(target_arch = "wasm32"))]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::rc::Rc;

use maestro_app::presentation_data::footer_data_provider::{FooterDataProvider, FooterOperations};
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

mod repository;
pub use repository::Repository;

/// Controlled process effects beside real isolated repository metadata.
pub struct MetadataEffects {
    pub repository: Repository,
    pub git: RefCell<Git>,
    pub git_dirs: RefCell<Vec<String>>,
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

impl MetadataEffects {
    pub fn new(files: BTreeMap<String, Entry>) -> Rc<Self> {
        Rc::new(Self {
            repository: Repository::new(files),
            git: RefCell::new(Git::Output(Some("main\n".to_owned()))),
            git_dirs: RefCell::default(),
        })
    }
    pub fn path(&self, path: &str) -> String {
        self.repository.path(path)
    }
    pub fn fail(&self, stat: &[String], read: &[String]) {
        for path in stat.iter().chain(read) {
            self.repository.unreadable(path);
        }
    }
    pub fn set(&self, path: &str, entry: Entry) {
        self.repository.set(path, entry);
    }
}

fn failure() -> io::Error {
    io::Error::other("controlled failure")
}

impl FooterOperations for MetadataEffects {
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
    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        self.git_dirs.borrow_mut().push(repo_dir.to_owned());
        match &*self.git.borrow() {
            Git::Output(output) => Ok(output.clone()),
            Git::SpawnError => Err(failure()),
        }
    }
}

/// A provider over `ops` for `cwd`.
pub fn provider(ops: &Rc<MetadataEffects>, cwd: &str) -> FooterDataProvider {
    FooterDataProvider::new(ops.path(cwd), Rc::clone(ops) as Rc<dyn FooterOperations>)
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
use crate::native;
#[cfg(unix)]
pub use native::{Scratch, git, install_executable, probe};

impl maestro_watch::fs_watch::WatchOperations for MetadataEffects {
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
