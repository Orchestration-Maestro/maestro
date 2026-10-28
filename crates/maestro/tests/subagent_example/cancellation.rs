//! Native launch observations and signal completion through the existing I/O fixture.
#![cfg(target_os = "linux")]
use super::*;
use maestro_extensions_wasmtime::native_io::{ChildProcess, IoFuture, NativeIo, SpawnOptions};
use std::cell::{Cell, RefCell};

/// Observes original native controls without replacing process execution or its pipes.
struct Launches {
    /// Application-owned process implementation.
    native: Rc<dyn NativeIo>,
    /// Admissions, including any erroneous queued launches after cancellation.
    count: Cell<usize>,
    /// Original native controls for independent settlement observations.
    children: RefCell<Vec<Rc<dyn ChildProcess>>>,
}
impl NativeIo for Launches {
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        self.count.set(self.count.get() + 1);
        Box::pin(async move {
            let child = self.native.spawn(command, args, options).await?;
            self.children.borrow_mut().push(Rc::clone(&child));
            Ok(child)
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String> {
        self.native.kill(pid, signal)
    }
    fn watch(
        &self,
        path: String,
        listener: Rc<dyn Fn()>,
    ) -> Result<Rc<dyn maestro_extensions_wasmtime::native_io::FsWatcher>, String> {
        self.native.watch(path, listener)
    }
    fn fetch(
        &self,
        request: maestro_extensions_wasmtime::native_io::Request,
    ) -> IoFuture<'_, Rc<dyn maestro_extensions_wasmtime::native_io::Response>> {
        self.native.fetch(request)
    }
}

/// Cancels the original signal if a fixture assertion unwinds early.
struct AbortOnDrop(maestro_app::Cancellation);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Completes once four distinct children have blocked their signal and reported readiness.
fn progress(
    send: tokio::sync::oneshot::Sender<()>,
) -> maestro_app::AgentToolUpdateCallback<Value, maestro_app::Local> {
    let send = RefCell::new(Some(send));
    Rc::new(move |partial| {
        let details = partial.details.unwrap();
        let results = details["results"].as_array().unwrap();
        assert!(results.iter().all(|result| result["exitCode"] == -1));
        let ready = results
            .iter()
            .filter(|result| {
                result["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|message| message["content"][0]["text"] == "ready")
            })
            .count();
        if ready == 4
            && let Some(send) = send.borrow_mut().take()
        {
            send.send(()).unwrap();
        }
    })
}

#[test]
fn subagent_parallel_limit_and_abort_reach_every_owned_child() {
    let component = component_build::example_component("maestro-subagents", "component").unwrap();
    if child_fixture(
        "subagent_example::cancellation::subagent_parallel_limit_and_abort_reach_every_owned_child",
        Some(&component),
    )
    .is_some()
    {
        return;
    }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            controlled_role(&root, "project/.maestro/agents");
            let launches = Rc::new(Launches {
                native: maestro_app::extension_native_io::extension_native_io(&local),
                count: Cell::new(0),
                children: RefCell::new(Vec::new()),
            });
            let session = crate::native_io::faults::session(&root, &local, launches.clone()).await;
            let signal = maestro_app::Cancellation::new();
            let _abort = AbortOnDrop(signal.clone());
            let aborted = signal.clone();
            let (send, ready) = tokio::sync::oneshot::channel();
            let active = Rc::clone(&session);
            let pending = tokio::task::spawn_local(async move {
                invoked(&active, json!({"agentScope":"project","confirmProjectAgents":false,"tasks":vec![json!({"agent":"role","task":"cancel"});8]}), Some(aborted), Some(progress(send))).await
            });
            ready.await.unwrap();
            let admitted_before_abort = launches.count.get();
            signal.abort();
            assert!(
                pending
                    .await
                    .unwrap()
                    .unwrap_err()
                    .contains("Subagent was aborted")
            );
            let children = launches.children.borrow().clone();
            let mut codes = Vec::new();
            for child in children {
                codes.push(child.wait().await.unwrap().code);
            }
            assert_eq!(admitted_before_abort, 4);
            assert_eq!(launches.count.get(), 4);
            assert_eq!(codes, vec![Some(0); 4]);
            preaborted(&session, &launches).await;
        })
    });
}

/// A signal cancelled before admission still terminates the newly owned child.
async fn preaborted(session: &Rc<AgentSession>, launches: &Launches) {
    let preaborted = maestro_app::Cancellation::new();
    preaborted.abort();
    let result = invoked(
        session,
        json!({"agentScope":"project","agent":"role","task":"cancel"}),
        Some(preaborted),
        None,
    )
    .await;
    assert!(result.unwrap_err().contains("Subagent was aborted"));
    let last = launches.children.borrow().last().cloned().unwrap();
    last.wait().await.unwrap();
    assert_eq!(launches.count.get(), 5);
}
