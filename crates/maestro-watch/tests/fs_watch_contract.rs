//! Shared helper behavior through replaceable watch effects.
use maestro_watch::fs_watch::{
    FsWatcher, WatchOperations, WatchTimer, close_watcher, watch_with_error_handler,
};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};

struct Handle {
    closes: Rc<Cell<usize>>,
    fails: bool,
}
impl FsWatcher for Handle {
    fn close(&mut self) -> io::Result<()> {
        self.closes.set(self.closes.get() + 1);
        if self.fails {
            Err(io::Error::other("close failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn close_handles_absence_and_close_failure() {
    close_watcher(None);
    for fails in [false, true] {
        let closes = Rc::new(Cell::new(0));
        close_watcher(Some(Box::new(Handle {
            closes: Rc::clone(&closes),
            fails,
        })));
        assert_eq!(closes.get(), 1);
    }
}

type Listener = Rc<dyn Fn(Option<String>)>;

#[derive(Default)]
struct Controlled {
    listener: RefCell<Option<Listener>>,
    error: RefCell<Option<Rc<dyn Fn()>>>,
    closes: Rc<Cell<usize>>,
    fails: bool,
}
impl WatchOperations for Controlled {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn FsWatcher>> {
        assert_eq!(path, "directory");
        if self.fails {
            return Err(io::Error::other("creation failed"));
        }
        *self.listener.borrow_mut() = Some(listener);
        *self.error.borrow_mut() = Some(error);
        Ok(Box::new(Handle {
            closes: Rc::clone(&self.closes),
            fails: false,
        }))
    }
    fn schedule(&self, _: Duration, _: Box<dyn FnOnce()>) -> Box<dyn WatchTimer> {
        unreachable!("helpers do not schedule retries")
    }
}

#[test]
fn watch_creation_and_error_delivery_share_the_seam() {
    let operations = Controlled::default();
    let names = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&names);
    let errors = Rc::new(Cell::new(0));
    let counted = Rc::clone(&errors);
    let on_error: Rc<dyn Fn()> = Rc::new(move || counted.set(counted.get() + 1));
    let watch = watch_with_error_handler(
        "directory",
        Rc::new(move |name| observed.borrow_mut().push(name)),
        Rc::clone(&on_error),
        &operations,
    );
    assert!(watch.is_some());
    assert!(names.borrow().is_empty());
    assert_eq!(errors.get(), 0);
    let listener = operations.listener.borrow().clone().unwrap();
    for name in [Some("a.json"), Some("A.json"), Some(""), None] {
        listener(name.map(str::to_owned));
    }
    assert_eq!(
        *names.borrow(),
        [
            Some("a.json".into()),
            Some("A.json".into()),
            Some(String::new()),
            None
        ]
    );
    let later_error = operations.error.borrow().clone().unwrap();
    later_error();
    later_error();
    assert_eq!(errors.get(), 2);
    assert_eq!(operations.closes.get(), 0);
    close_watcher(watch);
    assert_eq!(operations.closes.get(), 1);
    let failing = Controlled {
        fails: true,
        ..Controlled::default()
    };
    assert!(
        watch_with_error_handler("directory", Rc::new(|_| {}), Rc::clone(&on_error), &failing)
            .is_none()
    );
    assert_eq!(errors.get(), 3);
    native_creation(&on_error, &errors);
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
fn native_creation(on_error: &Rc<dyn Fn()>, errors: &Cell<usize>) {
    use maestro_watch::fs_watch::NativeWatchOperations;
    let root = std::env::temp_dir().join(format!("maestro-watch-contract-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    local.block_on(&runtime, async {
        let operations = NativeWatchOperations::new(Rc::clone(&local));
        assert!(
            watch_with_error_handler(
                root.join("missing").to_str().unwrap(),
                Rc::new(|_| {}),
                Rc::clone(on_error),
                &operations
            )
            .is_none()
        );
        assert_eq!(errors.get(), 4);
        let watcher = watch_with_error_handler(
            root.to_str().unwrap(),
            Rc::new(|_| {}),
            Rc::clone(on_error),
            &operations,
        );
        assert!(watcher.is_some());
        assert_eq!(errors.get(), 4);
        close_watcher(watcher);
    });
    std::fs::remove_dir(root).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn native_creation(_: &Rc<dyn Fn()>, _: &Cell<usize>) {}

#[test]
fn retry_delay_is_five_seconds() {
    assert_eq!(maestro_watch::fs_watch::FS_WATCH_RETRY_DELAY_MS, 5000);
}
