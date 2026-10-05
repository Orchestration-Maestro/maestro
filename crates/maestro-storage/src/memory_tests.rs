use super::MemoryStorage;
use crate::conformance;

#[test]
fn whole_batch_visibility() {
    let storage = MemoryStorage::new();
    conformance::whole_batch_visibility(&storage, &storage);
}

#[test]
fn close_settles_admitted_writes() {
    let storage = MemoryStorage::new();
    conformance::close_settles_admitted_writes(&storage, &storage);
}

#[test]
fn serial_mutations() {
    conformance::serial_mutations(&MemoryStorage::new());
}

impl crate::conformance::Controls for MemoryStorage {
    fn pause_next_append(&self, session_id: &str) -> crate::conformance::Pause {
        let (reached_tx, reached) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        self.sessions.lock().unwrap()[session_id]
            .hooks
            .lock()
            .unwrap()
            .pause = Some((reached_tx, release_rx));
        crate::conformance::Pause { reached, release }
    }
    fn observe_close_draining(
        &self,
        session_id: &str,
        witness: std::sync::mpsc::Sender<crate::conformance::CloseWitness>,
    ) {
        self.sessions.lock().unwrap()[session_id]
            .hooks
            .lock()
            .unwrap()
            .draining = Some(witness);
    }
    fn observe_write_settled(
        &self,
        session_id: &str,
        witness: std::sync::mpsc::Sender<crate::conformance::CloseWitness>,
    ) {
        self.sessions.lock().unwrap()[session_id]
            .hooks
            .lock()
            .unwrap()
            .settled = Some(witness);
    }
    fn observe_close(&self, session_id: &str) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.sessions.lock().unwrap()[session_id]
            .hooks
            .lock()
            .unwrap()
            .close = Some(tx);
        rx
    }
}
