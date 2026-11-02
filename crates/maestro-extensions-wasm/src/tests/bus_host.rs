//! Controlled bus imports and recorded payloads.
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
    /// Registered channel and callback pairs.
    pub listeners: Vec<(String, u32)>,
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
        self.listeners.push((channel.into(), id));
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
        self.listeners.retain(|(_, id)| *id != subscription.id);
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
            .listeners
            .iter()
            .filter(|(name, _)| name == channel)
            .map(|(_, id)| *id)
            .collect())
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
