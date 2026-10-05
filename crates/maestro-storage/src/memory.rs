use crate::{
    Record, RecordSession, SessionHeader, SessionMetadata, SessionSnapshot, Storage, StorageError,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};

/// Explicit ephemeral storage without configuration or I/O.
#[derive(Default)]
pub struct MemoryStorage {
    sessions: Mutex<BTreeMap<String, Arc<Shared>>>,
}

impl MemoryStorage {
    /// Creates an empty store; identities exist only within this instance's lifetime.
    pub fn new() -> Self {
        Self::default()
    }
}

struct Shared {
    snapshot: Mutex<SessionSnapshot>,
    mutation: Mutex<()>,
    #[cfg(test)]
    hooks: Mutex<Hooks>,
}

#[cfg(test)]
#[derive(Default)]
struct Hooks {
    pause: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    close: Option<std::sync::mpsc::Sender<()>>,
}

#[derive(Default)]
struct Admission {
    closed: bool,
    writes: usize,
}

struct Write<'a>(&'a Handle);
impl Drop for Write<'_> {
    fn drop(&mut self) {
        let mut admission = self.0.admission.lock().unwrap();
        admission.writes -= 1;
        self.0.drained.notify_all();
    }
}

struct Handle {
    state: Arc<Shared>,
    admission: Mutex<Admission>,
    drained: Condvar,
}

impl Handle {
    fn new(state: Arc<Shared>) -> Self {
        Self {
            state,
            admission: Mutex::new(Admission::default()),
            drained: Condvar::new(),
        }
    }
}

impl Storage for MemoryStorage {
    fn create(&self, header: SessionHeader) -> Result<Arc<dyn RecordSession>, StorageError> {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.contains_key(&header.session_id) {
            return Err(StorageError::Rejected {
                reason: "session already exists".into(),
            });
        }
        let id = header.session_id.clone();
        let state = Arc::new(Shared {
            snapshot: Mutex::new(SessionSnapshot {
                metadata: SessionMetadata {
                    header,
                    persistent_locator: None,
                    resumable: false,
                },
                records: vec![],
                selected_position: None,
            }),
            mutation: Mutex::new(()),
            #[cfg(test)]
            hooks: Mutex::new(Hooks::default()),
        });
        sessions.insert(id, state.clone());
        Ok(Arc::new(Handle::new(state)))
    }
    fn open(&self, session_id: &str) -> Result<Arc<dyn RecordSession>, StorageError> {
        let state = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .ok_or_else(|| StorageError::Rejected {
                reason: "unknown session".into(),
            })?;
        Ok(Arc::new(Handle::new(state)))
    }
    fn list(&self) -> Result<Vec<SessionMetadata>, StorageError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .map(|state| state.snapshot.lock().unwrap().metadata.clone())
            .collect())
    }
}

impl RecordSession for Handle {
    fn read(&self) -> Result<SessionSnapshot, StorageError> {
        let admission = self.admission.lock().unwrap();
        if admission.closed {
            return Err(StorageError::Closed);
        }
        Ok(self.state.snapshot.lock().unwrap().clone())
    }
    fn get(&self, record_id: &str) -> Result<Option<Record>, StorageError> {
        Ok(self
            .read()?
            .records
            .into_iter()
            .find(|record| record.id == record_id))
    }
    fn append(
        &self,
        records: Vec<Record>,
        selected_position: Option<String>,
    ) -> Result<(), StorageError> {
        let mut admission = self.admission.lock().unwrap();
        if admission.closed {
            return Err(StorageError::Closed);
        }
        admission.writes += 1;
        let _write = Write(self);
        drop(admission);
        let _mutation = self.state.mutation.lock().unwrap();
        let mut state = self.state.snapshot.lock().unwrap().clone();
        let mut ids: BTreeSet<_> = state
            .records
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        for record in &records {
            if !ids.insert(&record.id) {
                return Err(StorageError::Rejected {
                    reason: "duplicate record identity".into(),
                });
            }
        }
        if selected_position
            .as_deref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err(StorageError::Rejected {
                reason: "unknown selected position".into(),
            });
        }
        let mut records = records.into_iter();
        if let Some(first) = records.next() {
            state.records.push(first);
        }
        #[cfg(test)]
        let pause = self.state.hooks.lock().unwrap().pause.take();
        #[cfg(test)]
        if let Some((reached, release)) = pause {
            reached.send(()).unwrap();
            release.recv().unwrap();
        }
        state.records.extend(records);
        state.selected_position = selected_position;
        *self.state.snapshot.lock().unwrap() = state;
        Ok(())
    }
    fn select(&self, selected_position: Option<String>) -> Result<(), StorageError> {
        self.append(vec![], selected_position)
    }
    fn close(&self) -> Result<(), StorageError> {
        let mut admission = self.admission.lock().unwrap();
        admission.closed = true;
        #[cfg(test)]
        if let Some(observed) = self.state.hooks.lock().unwrap().close.take() {
            observed.send(()).unwrap();
        }
        while admission.writes != 0 {
            admission = self.drained.wait(admission).unwrap();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod memory_tests;
