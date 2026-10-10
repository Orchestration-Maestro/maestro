//! Shared controlled capture gates for package-manager worker witnesses.
use super::support;
use std::{cell::RefCell, rc::Rc};
/// Controls a capture at its first awaited probe for each source.
pub struct Gates {
    /// Completion permits indexed by selected source.
    pub release: RefCell<std::collections::BTreeMap<usize, tokio::sync::oneshot::Sender<()>>>,
    /// Admission witnesses from the producer.
    starts: tokio::sync::mpsc::UnboundedSender<usize>,
}
impl Gates {
    /// Creates one gate per source and observes admission at the capture boundary.
    pub fn install(
        effects: &support::Effects,
        count: usize,
    ) -> (Rc<Self>, tokio::sync::mpsc::UnboundedReceiver<usize>) {
        let (starts, receive) = tokio::sync::mpsc::unbounded_channel();
        let gates = Rc::new(Self {
            release: RefCell::default(),
            starts,
        });
        let (release, mut wait): (
            std::collections::BTreeMap<_, _>,
            std::collections::BTreeMap<_, _>,
        ) = (0..count)
            .map(|i| {
                let (tx, rx) = tokio::sync::oneshot::channel();
                ((i, tx), (i, rx))
            })
            .unzip();
        *gates.release.borrow_mut() = release;
        let wait = Rc::new(RefCell::new(std::mem::take(&mut wait)));
        let observed = gates.clone();
        *effects.capture_hook.borrow_mut() = Some(gated_capture(observed, wait));
        (gates, receive)
    }
    /// Releases exactly one admitted capture.
    pub fn finish(&self, index: usize) {
        let release = self.release.borrow_mut().remove(&index).unwrap();
        release.send(()).unwrap();
    }
}
/// Produces native-kind responses around one gated initial probe.
fn gated_capture(
    observed: Rc<Gates>,
    wait: Rc<RefCell<std::collections::BTreeMap<usize, tokio::sync::oneshot::Receiver<()>>>>,
) -> support::CaptureHook {
    Rc::new(move |command, args, cwd| {
        let initial = command != "git" || args == ["rev-parse", "HEAD"];
        if !initial {
            let text = if args[0] == "rev-parse" {
                String::new()
            } else {
                format!("{}\tHEAD", "b".repeat(40))
            };
            return Box::pin(async move { Ok(text) });
        }
        let label = if command == "git" {
            cwd.unwrap().rsplit('/').next().unwrap()
        } else {
            args[1].as_str()
        };
        let index: usize = label.strip_prefix('p').unwrap().parse().unwrap();
        let gate = wait.borrow_mut().remove(&index).unwrap();
        let starts = observed.starts.clone();
        let text = if command == "git" {
            "a".repeat(40)
        } else {
            "\"2\"".into()
        };
        Box::pin(async move {
            starts.send(index).unwrap();
            gate.await.unwrap();
            Ok(text)
        })
    })
}
/// Native local runtime used by concurrency witnesses.
pub fn driver(effects: &support::Effects) -> (tokio::runtime::Runtime, Rc<tokio::task::LocalSet>) {
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    (
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
        local,
    )
}
/// Bounds a broken asynchronous witness without relying on a delay for correctness.
pub async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("controlled completion witness did not fire")
}
