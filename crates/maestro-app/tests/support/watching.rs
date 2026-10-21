//! Controlled notifications, completions and closed timer queues.
use maestro_app::presentation_data::footer_data_provider::{FooterFileStats, FooterOperations};
use maestro_watch::fs_watch::{FsWatcher, WatchOperations, WatchTimer};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};

pub type Finish = Box<dyn FnOnce(io::Result<Option<String>>)>;
pub type PollListener = Rc<dyn Fn(FooterFileStats, FooterFileStats)>;
struct Watch {
    closed: Rc<Cell<bool>>,
    path: String,
    listener: Rc<dyn Fn(Option<String>)>,
    error: Rc<dyn Fn()>,
}
struct Timer {
    at: u64,
    cancelled: Rc<Cell<bool>>,
    callback: Option<Box<dyn FnOnce()>>,
}
pub struct Poll {
    pub closed: Rc<Cell<bool>>,
    pub path: String,
    pub interval: Duration,
    listener: PollListener,
}
struct Cancel {
    closed: Rc<Cell<bool>>,
    fails: bool,
}
impl WatchTimer for Cancel {
    fn cancel(&mut self) {
        self.closed.set(true);
    }
}
impl FsWatcher for Cancel {
    fn close(&mut self) -> io::Result<()> {
        self.closed.set(true);
        if self.fails {
            Err(io::Error::other("controlled close"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Cancel {
    fn drop(&mut self) {
        self.closed.set(true);
    }
}
#[derive(Default)]
pub struct Effects {
    pub repository: crate::footer_support::Repository,
    time: Cell<u64>,
    timers: RefCell<Vec<Timer>>,
    watches: RefCell<Vec<Watch>>,
    pub queries: RefCell<Vec<Finish>>,
    pub query_dirs: RefCell<Vec<String>>,
    pub sync_calls: Cell<usize>,
    pub attempts: Cell<usize>,
    pub failures: RefCell<Vec<usize>>,
    pub close_failure: Cell<usize>,
    pub polls: RefCell<Vec<Poll>>,
}
impl Effects {
    pub fn repo() -> Rc<Self> {
        let e = Rc::new(Self::default());
        for path in ["/repo/.git", "/repo/.git/reftable", "/other/.git"] {
            e.directory(path);
        }
        e.text("/repo/.git/HEAD", "ref: refs/heads/.invalid");
        e.text("/repo/.git/reftable/tables.list", "0");
        e.text("/other/.git/HEAD", "ref: refs/heads/other");
        e
    }
    pub fn path(&self, path: &str) -> String {
        self.repository.path(path)
    }
    pub fn paths<const N: usize>(&self, paths: [&str; N]) -> Vec<String> {
        paths.map(|path| self.path(path)).into()
    }
    pub fn directory(&self, path: &str) {
        self.repository.directory(path);
    }
    pub fn text(&self, path: &str, value: &str) {
        self.repository.text(path, value);
    }
    pub fn event(&self, index: usize, name: Option<&str>) {
        let cb = Rc::clone(&self.watches.borrow()[index].listener);
        cb(name.map(str::to_owned));
    }
    pub fn error(&self, index: usize) {
        let cb = Rc::clone(&self.watches.borrow()[index].error);
        cb();
    }
    pub fn poll(&self, index: usize, now: FooterFileStats, previous: FooterFileStats) {
        let cb = Rc::clone(&self.polls.borrow()[index].listener);
        cb(now, previous);
    }
    pub fn advance(&self, delta: u64) {
        let end = self.time.get() + delta;
        loop {
            let timer = {
                let mut timers = self.timers.borrow_mut();
                let next = timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.at <= end && !t.cancelled.get() && t.callback.is_some())
                    .min_by_key(|(_, t)| t.at)
                    .map(|(i, _)| i);
                next.map(|i| (timers[i].at, timers[i].callback.take().unwrap()))
            };
            let Some((at, cb)) = timer else { break };
            self.time.set(at);
            cb();
        }
        self.time.set(end);
    }
    pub fn finish(&self, value: io::Result<Option<String>>) {
        let cb = self.queries.borrow_mut().remove(0);
        cb(value);
    }
    pub fn answer(&self, value: &str) {
        self.finish(Ok(Some(value.into())));
    }
    pub fn active(&self) -> Vec<String> {
        self.watches
            .borrow()
            .iter()
            .filter(|w| !w.closed.get())
            .map(|w| w.path.clone())
            .collect()
    }
    pub fn live_polls(&self) -> usize {
        self.polls
            .borrow()
            .iter()
            .filter(|p| !p.closed.get())
            .count()
    }
    pub fn pending_timers(&self) -> usize {
        self.timers
            .borrow()
            .iter()
            .filter(|t| !t.cancelled.get() && t.callback.is_some())
            .count()
    }
    pub fn teardown(&self) {
        let timers = std::mem::take(&mut *self.timers.borrow_mut());
        let watches = std::mem::take(&mut *self.watches.borrow_mut());
        let polls = std::mem::take(&mut *self.polls.borrow_mut());
        let queries = std::mem::take(&mut *self.queries.borrow_mut());
        drop((timers, watches, polls, queries));
    }
}
impl WatchOperations for Effects {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn FsWatcher>> {
        let attempt = self.attempts.get() + 1;
        self.attempts.set(attempt);
        if self.failures.borrow().contains(&attempt) {
            return Err(io::Error::other("controlled registration"));
        }
        let closed = Rc::new(Cell::new(false));
        self.watches.borrow_mut().push(Watch {
            closed: closed.clone(),
            path: path.to_owned(),
            listener,
            error,
        });
        Ok(Box::new(Cancel {
            closed,
            fails: self.close_failure.get() == attempt,
        }))
    }
    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn WatchTimer> {
        let closed = Rc::new(Cell::new(false));
        self.timers.borrow_mut().push(Timer {
            at: self.time.get() + u64::try_from(delay.as_millis()).unwrap(),
            cancelled: closed.clone(),
            callback: Some(callback),
        });
        Box::new(Cancel {
            closed,
            fails: false,
        })
    }
}
impl FooterOperations for Effects {
    fn exists(&self, path: &str) -> bool {
        std::fs::exists(path).unwrap_or(false)
    }
    fn symbolic_ref_sync(&self, _: &str) -> io::Result<Option<String>> {
        self.sync_calls.set(self.sync_calls.get() + 1);
        Ok(Some("main\n".into()))
    }
    fn symbolic_ref_async(&self, repo: &str, callback: Finish) {
        self.query_dirs.borrow_mut().push(repo.to_owned());
        self.queries.borrow_mut().push(callback);
    }
    fn watch_file(
        &self,
        path: &str,
        interval: Duration,
        listener: PollListener,
    ) -> Box<dyn FsWatcher> {
        let closed = Rc::new(Cell::new(false));
        self.polls.borrow_mut().push(Poll {
            closed: closed.clone(),
            path: path.to_owned(),
            interval,
            listener,
        });
        Box::new(Cancel {
            closed,
            fails: false,
        })
    }
}
