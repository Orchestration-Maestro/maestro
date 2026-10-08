#![cfg(test)]
use maestro_extensions::{EventBusController, EventListener, TailSpawner, create_event_bus};
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub type Work = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type Trace = Arc<Mutex<Vec<Value>>>;
#[derive(Clone)]
pub struct Env {
    pub bus: EventBusController,
    pub trace: Trace,
    pub errors: Arc<Mutex<Vec<String>>>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    queue: mpsc::UnboundedSender<Command>,
}
enum Command {
    Work(Work),
    Barrier(oneshot::Sender<()>),
}
impl Env {
    fn new(queued: bool) -> Self {
        require_send_sync::<maestro_extensions::EventBus>();
        require_send_sync::<EventBusController>();
        require_send_sync::<maestro_extensions::Subscription>();
        let tasks = Arc::new(Mutex::new(Vec::new()));
        let (queue, receiver) = mpsc::unbounded_channel();
        let worker_tasks = tasks.clone();
        tokio::spawn(dispatch(receiver, worker_tasks));
        let spawn: TailSpawner = if queued {
            let sender = queue.clone();
            Arc::new(move |work| sender.send(Command::Work(work)).unwrap())
        } else {
            let tasks = tasks.clone();
            Arc::new(move |work| {
                let task = tokio::spawn(work);
                tasks.lock().unwrap().push(task);
            })
        };
        let errors = Arc::new(Mutex::new(Vec::new()));
        let reporter = errors.clone();
        Self {
            bus: create_event_bus(
                spawn,
                Arc::new(move |line| reporter.lock().unwrap().push(line.to_owned())),
            ),
            trace: Arc::default(),
            errors,
            tasks,
            queue,
        }
    }
    pub fn mark(&self, value: impl Into<Value>) {
        mark(&self.trace, value);
    }
    pub fn listener(&self, value: &'static str) -> EventListener {
        let trace = self.trace.clone();
        Arc::new(move |_, _| {
            mark(&trace, value);
            Ok(None)
        })
    }
    pub async fn settle(&self) {
        loop {
            let (send, receive) = oneshot::channel();
            self.queue.send(Command::Barrier(send)).unwrap();
            receive.await.unwrap();
            let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
            if tasks.is_empty() {
                break;
            }
            for task in tasks {
                task.await.unwrap();
            }
        }
    }
    pub fn verify(&self, id: &str) {
        let expected = case(id)["expected"].clone();
        let trace = json!(*self.trace.lock().unwrap());
        let errors = self.errors.lock().unwrap();
        if expected == "inert" {
            assert_eq!(trace, json!([]), "{id}");
            assert!(errors.is_empty(), "{id}: {errors:?}");
            eprintln!("verified corpus case: {id}");
            return;
        }
        let actual = if expected.is_object() {
            if expected.get("trace").is_some() {
                json!({"trace": trace, "errors": *errors})
            } else {
                assert_eq!(trace, json!([]));
                json!({"errors": *errors})
            }
        } else {
            assert!(errors.is_empty(), "{id}: {errors:?}");
            trace
        };
        assert_eq!(actual, expected, "{id}");
        eprintln!("verified corpus case: {id}");
    }
}
pub fn mark(trace: &Trace, value: impl Into<Value>) {
    trace.lock().unwrap().push(value.into());
}
pub fn case(id: &str) -> Value {
    let all: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/event_traces.json")).unwrap();
    all.into_iter().find(|case| case["id"] == id).unwrap()
}
pub fn run<F: Future<Output = ()>>(test: impl Fn(Env) -> F) {
    for queued in [false, true] {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                let env = Env::new(queued);
                test(env.clone()).await;
                env.settle().await;
            });
    }
}

async fn dispatch(
    mut receiver: mpsc::UnboundedReceiver<Command>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Work(work) => {
                let task = tokio::spawn(work);
                tasks.lock().unwrap().push(task);
            }
            Command::Barrier(done) => {
                done.send(()).unwrap();
            }
        }
    }
}

fn require_send_sync<T: Send + Sync>() {}
