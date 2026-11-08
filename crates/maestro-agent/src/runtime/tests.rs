//! Immediate nonblocking probes for queue retirement.
use super::{Agent, AgentOptions};
use crate::{AgentMessage, CustomAgentMessages};
use std::sync::{
    Arc, RwLock, Weak,
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
