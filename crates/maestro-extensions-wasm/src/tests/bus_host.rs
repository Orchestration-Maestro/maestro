//! Controlled bus imports and recorded payloads.
use maestro_listeners::Listeners;
use serde_json::Value;
use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// One callback registered on a channel; each registration is its own record.
struct Registration {
    /// Channel the callback listens on.
    channel: String,
    /// Callback identity; several registrations may share it.
    id: u32,
}

/// Host subscription points to one callback identity.
pub struct Subscription {
    /// Callback identity.
    pub id: u32,
    /// Shared destructor count.
    drops: Arc<AtomicUsize>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
/// Invocation-local emitter resource.
pub struct Emitter {
    /// Invocation identity supplied by the renderer driver.
    pub key: usize,
}
/// Shared bus observations and controlled failures.
#[derive(Default)]
pub struct Bank {
    /// Resource destruction, separate from explicit removal.
    pub subscription_drops: Arc<AtomicUsize>,
    /// Ordinary and callback emissions.
    pub emissions: Vec<(String, Value)>,
    /// Actual emitter resource and attempted payload, including rejected emission.
    pub renderer_emissions: Vec<(usize, String, Value)>,
    /// Registrations in order; reading takes a snapshot, so a shared bank can be read.
    registrations: RefCell<Listeners<Arc<Registration>>>,
    /// Emit-time callback identities retained for the generated local walk.
    pub snapshots: Vec<Option<Vec<u32>>>,
    /// Announced independent tails.
    pub tails: Vec<u32>,
    /// Announced tails not yet handed to teardown.
    pub unreleased: Vec<u32>,
    /// Registration rejection.
    pub registration_error: Option<String>,
    /// Holds asynchronous emissions before their listeners run.
    pub emission_holds: std::collections::VecDeque<super::observed::Hold>,
    /// Emission rejection.
    pub emission_error: Option<String>,
    /// Removal rejection.
    pub removal_error: Option<String>,
    /// Announcement rejection.
    pub tail_error: Option<String>,
    /// Explicit removals, including repeated calls.
    pub removals: Vec<u32>,
}
impl Bank {
    /// Registers a callback without invoking it.
    pub fn on(&mut self, channel: &str, id: u32) -> Result<Subscription, String> {
        if let Some(error) = &self.registration_error {
            return Err(error.clone());
        }
        self.registrations.borrow_mut().add(Arc::new(Registration {
            channel: channel.into(),
            id,
        }));
        Ok(Subscription {
            id,
            drops: Arc::clone(&self.subscription_drops),
        })
    }
    /// Removes this registration, preserving independent tails.
    pub fn unsubscribe(&mut self, subscription: &Subscription) -> Result<(), String> {
        if let Some(error) = &self.removal_error {
            return Err(error.clone());
        }
        self.removals.push(subscription.id);
        for registration in self.snapshot() {
            if registration.id == subscription.id {
                self.registrations
                    .borrow_mut()
                    .remove(Arc::as_ptr(&registration));
            }
        }
        Ok(())
    }
    /// Records data and captures the matching callbacks for controlled delivery.
    pub fn emit(&mut self, channel: &str, data: &str) -> Result<Vec<u32>, String> {
        if let Some(error) = &self.emission_error {
            return Err(error.clone());
        }
        self.emissions.push((
            channel.into(),
            serde_json::from_str(data).map_err(|e| e.to_string())?,
        ));
        Ok(self
            .snapshot()
            .iter()
            .filter(|registration| registration.channel == channel)
            .map(|registration| registration.id)
            .collect())
    }
    /// Removes the first registration on `channel` and returns its callback identity.
    pub fn take_first(&mut self, channel: &str) -> Option<u32> {
        let first = self
            .snapshot()
            .into_iter()
            .find(|registration| registration.channel == channel)?;
        self.registrations.borrow_mut().remove(Arc::as_ptr(&first));
        Some(first.id)
    }
    /// Registered channel and callback identity pairs, in registration order.
    pub fn listeners(&self) -> Vec<(String, u32)> {
        self.snapshot()
            .iter()
            .map(|registration| (registration.channel.clone(), registration.id))
            .collect()
    }
    /// The live registrations, retained independently of later edits.
    fn snapshot(&self) -> Vec<Arc<Registration>> {
        self.registrations.borrow_mut().snapshot()
    }
    /// Announces a tail without polling it.
    pub fn tail(&mut self, id: u32) -> Result<(), String> {
        if let Some(error) = &self.tail_error {
            return Err(error.clone());
        }
        self.tails.push(id);
        self.unreleased.push(id);
        Ok(())
    }
}
