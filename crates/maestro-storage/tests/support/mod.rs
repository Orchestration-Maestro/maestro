use maestro_storage::{
    Record, RecordSession, SessionHeader, SessionMetadata, SessionSnapshot, Storage, StorageError,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, RwLock};

#[derive(Default)]
pub struct ControlledStorage {
    sessions: Mutex<HashMap<String, Arc<Data>>>,
    fail_paused_read: bool,
    fail_closed_read: bool,
    early_close: bool,
}
impl ControlledStorage {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn failing_reads(closed: bool) -> Self {
        Self {
            fail_paused_read: !closed,
            fail_closed_read: closed,
            ..Self::default()
        }
    }
    pub fn early_close() -> Self {
        Self {
            early_close: true,
            ..Self::default()
        }
    }
    pub fn observe_next_mutation(&self, id: &str) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .admission = Some(tx);
        rx
    }
    pub fn arm_uncertain(&self, id: &str, after: bool) {
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .uncertain = Some(after);
    }
}
#[derive(Default)]
struct Schedule {
    pause: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    admission: Option<std::sync::mpsc::Sender<()>>,
    uncertain: Option<bool>,
    closing: Option<std::sync::mpsc::Sender<()>>,
    paused: bool,
    draining: Option<std::sync::mpsc::Sender<maestro_storage::conformance::CloseWitness>>,
    settled: Option<std::sync::mpsc::Sender<maestro_storage::conformance::CloseWitness>>,
}
struct Data {
    fail_paused_read: bool,
    fail_closed_read: bool,
    early_close: bool,
    snapshot: RwLock<SessionSnapshot>,
    writing: Mutex<()>,
    controls: Mutex<Schedule>,
}
#[derive(Default)]
struct Life {
    closed: bool,
    uncertain: bool,
    active: usize,
}
struct Open {
    data: Arc<Data>,
    life: Mutex<Life>,
    settled: Condvar,
}
impl Open {
    fn new(data: Arc<Data>) -> Self {
        Self {
            data,
            life: Mutex::new(Life::default()),
            settled: Condvar::new(),
        }
    }
}
struct Active<'a>(&'a Open);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        let mut life = self.0.life.lock().unwrap();
        if let Some(witness) = self.0.data.controls.lock().unwrap().settled.take() {
            let _ = witness.send(maestro_storage::conformance::CloseWitness::WriteSettled);
        }
        life.active -= 1;
        self.0.settled.notify_all();
    }
}
fn rejected(reason: &str) -> StorageError {
    StorageError::Rejected {
        reason: reason.into(),
    }
}
impl Storage for ControlledStorage {
    fn create(&self, header: SessionHeader) -> Result<Arc<dyn RecordSession>, StorageError> {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.contains_key(&header.session_id) {
            return Err(rejected("existing session"));
        }
        let id = header.session_id.clone();
        let data = Arc::new(Data {
            fail_paused_read: self.fail_paused_read,
            fail_closed_read: self.fail_closed_read,
            early_close: self.early_close,
            snapshot: RwLock::new(SessionSnapshot {
                metadata: SessionMetadata {
                    header,
                    persistent_locator: Some(format!("synthetic:{id}")),
                    resumable: false,
                },
                records: vec![],
                selected_position: None,
            }),
            writing: Mutex::new(()),
            controls: Mutex::new(Schedule::default()),
        });
        sessions.insert(id, data.clone());
        Ok(Arc::new(Open::new(data)))
    }
    fn open(&self, id: &str) -> Result<Arc<dyn RecordSession>, StorageError> {
        let data = self
            .sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| rejected("unknown session"))?;
        Ok(Arc::new(Open::new(data)))
    }
    fn list(&self) -> Result<Vec<SessionMetadata>, StorageError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .map(|data| data.snapshot.read().unwrap().metadata.clone())
            .collect())
    }
}
impl RecordSession for Open {
    fn read(&self) -> Result<SessionSnapshot, StorageError> {
        let life = self.life.lock().unwrap();
        if life.closed && !self.data.fail_closed_read {
            return Err(StorageError::Closed);
        }
        if self.data.fail_paused_read && self.data.controls.lock().unwrap().paused {
            return Err(rejected("deliberately failing paused read"));
        }
        Ok(self.data.snapshot.read().unwrap().clone())
    }
    fn get(&self, id: &str) -> Result<Option<Record>, StorageError> {
        Ok(self
            .read()?
            .records
            .into_iter()
            .find(|record| record.id == id))
    }
    fn append(&self, records: Vec<Record>, position: Option<String>) -> Result<(), StorageError> {
        let mut life = self.life.lock().unwrap();
        if life.closed {
            return Err(StorageError::Closed);
        }
        if life.uncertain {
            return Err(StorageError::Uncertain {
                reason: "handle cannot mutate".into(),
            });
        }
        life.active += 1;
        let _active = Active(self);
        drop(life);
        if let Some(observed) = self.data.controls.lock().unwrap().admission.take() {
            observed.send(()).unwrap();
        }
        let _writing = self.data.writing.lock().unwrap();
        if self.life.lock().unwrap().uncertain {
            return Err(StorageError::Uncertain {
                reason: "prior unknown outcome".into(),
            });
        }
        let mut proposed = self.data.snapshot.read().unwrap().clone();
        let mut ids: HashSet<_> = proposed
            .records
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        for record in &records {
            if !ids.insert(&record.id) {
                return Err(rejected("duplicate record"));
            }
        }
        if position.as_deref().is_some_and(|id| !ids.contains(id)) {
            return Err(rejected("unknown position"));
        }
        let mut incoming = records.into_iter();
        if let Some(first) = incoming.next() {
            proposed.records.push(first);
        }
        let pause = self.data.controls.lock().unwrap().pause.take();
        if let Some((reached, release)) = pause {
            self.data.controls.lock().unwrap().paused = true;
            reached.send(()).unwrap();
            release.recv().unwrap();
        }
        proposed.records.extend(incoming);
        proposed.selected_position = position;
        let fault = self.data.controls.lock().unwrap().uncertain.take();
        if fault != Some(false) {
            *self.data.snapshot.write().unwrap() = proposed;
        }
        if fault.is_some() {
            self.life.lock().unwrap().uncertain = true;
            return Err(StorageError::Uncertain {
                reason: "injected unknown outcome".into(),
            });
        }
        Ok(())
    }
    fn select(&self, position: Option<String>) -> Result<(), StorageError> {
        self.append(vec![], position)
    }
    fn close(&self) -> Result<(), StorageError> {
        let mut life = self.life.lock().unwrap();
        life.closed = true;
        if let Some(closing) = self.data.controls.lock().unwrap().closing.take() {
            closing.send(()).unwrap();
        }
        if self.data.early_close {
            return Ok(());
        }
        if life.active != 0
            && let Some(witness) = self.data.controls.lock().unwrap().draining.take()
        {
            let _ = witness.send(maestro_storage::conformance::CloseWitness::CloseDraining);
        }
        while life.active != 0 {
            life = self.settled.wait(life).unwrap();
        }
        Ok(())
    }
}

impl maestro_storage::conformance::Controls for ControlledStorage {
    fn pause_next_append(&self, id: &str) -> maestro_storage::conformance::Pause {
        let (tx, reached) = std::sync::mpsc::channel();
        let (release, rx) = std::sync::mpsc::channel();
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .pause = Some((tx, rx));
        maestro_storage::conformance::Pause { reached, release }
    }
    fn observe_close_draining(
        &self,
        id: &str,
        witness: std::sync::mpsc::Sender<maestro_storage::conformance::CloseWitness>,
    ) {
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .draining = Some(witness);
    }
    fn observe_write_settled(
        &self,
        id: &str,
        witness: std::sync::mpsc::Sender<maestro_storage::conformance::CloseWitness>,
    ) {
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .settled = Some(witness);
    }
    fn observe_close(&self, id: &str) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.sessions.lock().unwrap()[id]
            .controls
            .lock()
            .unwrap()
            .closing = Some(tx);
        rx
    }
}
