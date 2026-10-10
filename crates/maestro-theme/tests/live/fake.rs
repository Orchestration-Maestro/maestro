//! Controlled watch and timer effects with a manual clock.
use maestro_theme::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;
use std::time::Duration;

/// Listener of a watch, dropped when the watch closes.
type Listener = Rc<dyn Fn(Option<String>)>;

/// One created watch.
pub struct Watch {
    /// Watched directory.
    pub path: String,
    /// Notification listener until the watch closes.
    listener: RefCell<Option<Listener>>,
    /// Error callback until the watch closes.
    on_error: RefCell<Option<Rc<dyn Fn()>>>,
    /// Whether close was called.
    pub closed: Cell<bool>,
}

/// One scheduled callback.
struct Timer {
    /// Clock time at which it runs.
    due: u64,
    /// Callback until it ran or was cancelled.
    callback: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// Watches and timers under a manual clock.
#[derive(Default)]
pub struct Fake {
    /// Milliseconds elapsed.
    now: Cell<u64>,
    /// Created watches in order.
    pub watches: RefCell<Vec<Rc<Watch>>>,
    /// Scheduled timers in order.
    timers: RefCell<Vec<Rc<Timer>>>,
    /// Make `watch` fail.
    pub fail_create: Cell<bool>,
    /// Make `close` fail.
    pub fail_close: Cell<bool>,
    /// Number of cancelled timers that had not run.
    pub cancelled: Rc<Cell<usize>>,
}

impl Fake {
    /// A fresh clock at zero.
    pub fn new() -> Rc<Self> {
        Rc::default()
    }

    /// Deliver a notification to watch `index`.
    pub fn emit(&self, index: usize, filename: Option<&str>) {
        let watch = Rc::clone(&self.watches.borrow()[index]);
        let listener = watch.listener.borrow().clone();
        if let Some(listener) = listener {
            listener(filename.map(str::to_owned));
        }
    }

    /// Deliver an asynchronous failure to watch `index`.
    pub fn fail(&self, index: usize) {
        let watch = Rc::clone(&self.watches.borrow()[index]);
        let on_error = watch.on_error.borrow().clone();
        if let Some(on_error) = on_error {
            on_error();
        }
    }

    /// Timers that neither ran nor were cancelled.
    pub fn pending(&self) -> usize {
        self.timers
            .borrow()
            .iter()
            .filter(|timer| timer.callback.borrow().is_some())
            .count()
    }

    /// Move the clock forward, running due timers in time order.
    pub fn advance(&self, millis: u64) {
        let target = self.now.get() + millis;
        loop {
            let next = self
                .timers
                .borrow()
                .iter()
                .filter(|timer| timer.callback.borrow().is_some() && timer.due <= target)
                .min_by_key(|timer| timer.due)
                .cloned();
            let Some(timer) = next else { break };
            self.now.set(timer.due);
            let callback = timer.callback.borrow_mut().take();
            if let Some(callback) = callback {
                callback();
            }
        }
        self.now.set(target);
    }
}

impl ThemeWatchOperations for Fake {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn ThemeWatcher>> {
        if self.fail_create.get() {
            return Err(io::Error::other("create failed"));
        }
        let watch = Rc::new(Watch {
            path: path.to_owned(),
            listener: RefCell::new(Some(listener)),
            on_error: RefCell::new(Some(on_error)),
            closed: Cell::new(false),
        });
        self.watches.borrow_mut().push(Rc::clone(&watch));
        Ok(Box::new(Handle {
            watch,
            fail: self.fail_close.get(),
        }))
    }

    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn ThemeReloadTimer> {
        let timer = Rc::new(Timer {
            due: self.now.get() + u64::try_from(delay.as_millis()).unwrap(),
            callback: RefCell::new(Some(callback)),
        });
        self.timers.borrow_mut().push(Rc::clone(&timer));
        Box::new(Cancel {
            timer,
            cancelled: Rc::clone(&self.cancelled),
        })
    }
}

/// Close handle of a [`Watch`].
struct Handle {
    /// Watch to close.
    watch: Rc<Watch>,
    /// Report a close failure after closing.
    fail: bool,
}

impl ThemeWatcher for Handle {
    fn close(&mut self) -> io::Result<()> {
        self.watch.closed.set(true);
        self.watch.listener.borrow_mut().take();
        self.watch.on_error.borrow_mut().take();
        if self.fail {
            return Err(io::Error::other("close failed"));
        }
        Ok(())
    }
}

/// Cancel handle of a [`Timer`].
struct Cancel {
    /// Timer to cancel.
    timer: Rc<Timer>,
    /// Shared count of cancellations that took effect.
    cancelled: Rc<Cell<usize>>,
}

impl ThemeReloadTimer for Cancel {
    fn cancel(&mut self) {
        if self.timer.callback.borrow_mut().take().is_some() {
            self.cancelled.set(self.cancelled.get() + 1);
        }
    }
}
