//! Immediate nonblocking probes for queue retirement.
use super::{Agent, AgentOptions, lock, read, write};
use crate::{AgentMessage, CustomAgentMessages};
use std::sync::{
    Arc, Mutex, RwLock, Weak,
    atomic::{AtomicBool, Ordering},
};
/// Payload whose destructor observes the private lock boundary.
struct Probe {
    /// Weak owner avoids a retained callback cycle.
    owner: Weak<Agent<Self>>,
    /// Completion witness after the guard checks.
    observed: Arc<AtomicBool>,
}
impl CustomAgentMessages for Probe {
    fn role(&self) -> &'static str {
        "guard-probe"
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        let owner = self.owner.upgrade().unwrap();
        assert!(
            owner.steering.try_write().is_ok(),
            "steering guard must be released before payload drop"
        );
        assert!(
            owner.follow_up.try_write().is_ok(),
            "follow-up guard must be released before payload drop"
        );
        assert!(
            owner.state.try_write().is_ok(),
            "state guard must be released before payload drop"
        );
        self.observed.store(true, Ordering::SeqCst);
    }
}
/// Clearing or resetting retires queue payloads outside every internally held guard.
#[test]
fn queue_retirement_releases_guards_before_payload_drop() {
    let owner = Arc::new(Agent::new(AgentOptions::default()));
    for action in 0..4 {
        let observed = Arc::new(AtomicBool::new(false));
        let entry = AgentMessage::Custom(Arc::new(RwLock::new(Probe {
            owner: Arc::downgrade(&owner),
            observed: Arc::clone(&observed),
        })));
        match action {
            0 => {
                owner.steer(entry);
                owner.clear_steering_queue();
            }
            1 => {
                owner.follow_up(entry);
                owner.clear_follow_up_queue();
            }
            2 => {
                owner.steer(entry);
                owner.clear_all_queues();
            }
            _ => {
                owner.follow_up(entry);
                owner.reset();
            }
        }
        assert!(
            observed.load(Ordering::SeqCst),
            "destructor witness must complete before returning"
        );
    }
}

/// A panicking writer leaves the value the agent's `read` recovers.
#[test]
fn read_recovers_the_value_a_panicking_writer_left() {
    let shared = Arc::new(RwLock::new(5));
    assert_eq!(*read(&shared), 5);
    let writer = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = writer.write().unwrap();
        panic!("writer panics while holding the write guard");
    })
    .join();
    assert_eq!(*read(&shared), 5);
}

/// A write after a panicked writer lands for the agent's next `read`.
#[test]
fn write_lands_after_a_panicking_writer() {
    let shared = Arc::new(RwLock::new(1));
    *write(&shared) = 2;
    assert_eq!(*read(&shared), 2);
    let writer = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = writer.write().unwrap();
        panic!("writer panics while holding the write guard");
    })
    .join();
    assert_eq!(*write(&shared), 2);
    *write(&shared) = 3;
    assert_eq!(*read(&shared), 3);
}

/// A lock finds the value a panicked holder left in the mutex.
#[test]
fn lock_recovers_the_mutex_a_panicking_holder_left() {
    let shared = Arc::new(Mutex::new(9));
    assert_eq!(*lock(&shared), 9);
    let holder = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = holder.lock().unwrap();
        panic!("holder panics while holding the mutex guard");
    })
    .join();
    assert_eq!(*lock(&shared), 9);
    *lock(&shared) = 10;
    assert_eq!(*lock(&shared), 10);
}
