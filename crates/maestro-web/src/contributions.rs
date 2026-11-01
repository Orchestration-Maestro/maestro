//! Built-in and package contributions mounted at named slots.
use crate::dom::DomContent;
pub use maestro_app::contributions::{
    BrowserContribution, BrowserModule, ContributionDescriptor, ContributionSlot,
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

#[cfg(target_arch = "wasm32")]
mod adapters;
#[cfg(target_arch = "wasm32")]
mod builtins;
#[cfg(target_arch = "wasm32")]
pub(crate) use builtins::register_builtin_elements;
mod failure;
#[cfg(target_arch = "wasm32")]
mod host;
#[cfg(target_arch = "wasm32")]
mod loader;
#[cfg(target_arch = "wasm32")]
mod slot;
pub use failure::ContributionFailure;
#[cfg(target_arch = "wasm32")]
pub use loader::{ContributionHost, ModuleRegistrations};
#[cfg(target_arch = "wasm32")]
pub use slot::SlotMount;

/// Content showing one contribution for a slot's input.
pub trait ContributionView {
    /// Content for `input`.
    fn content(&self, input: &Value) -> Box<dyn DomContent>;
}
/// One registered contribution.
#[derive(Clone)]
struct Entry {
    /// Supplying package and its declaration.
    contribution: BrowserContribution,
    /// Content factory.
    view: Rc<dyn ContributionView>,
    /// Registration identity; removal requires the same identity.
    owner: Rc<()>,
}
thread_local! {
    /// Page contributions in registration order.
    static CONTRIBUTIONS: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    /// Provided host API names in provision order.
    static PROVIDED_APIS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}
/// Ownership of one registration; removal never erases a later replacement.
#[must_use]
pub struct ContributionRegistration {
    /// Package and identifier.
    key: (String, String),
    /// Identity shared with the stored entry.
    owner: Rc<()>,
}
/// Register or replace a contribution with the same package and identifier.
pub fn register_contribution(
    contribution: BrowserContribution,
    view: Rc<dyn ContributionView>,
) -> ContributionRegistration {
    let key = (
        contribution.package.clone(),
        contribution.descriptor.id.clone(),
    );
    let owner = Rc::new(());
    let entry = Entry {
        contribution,
        view,
        owner: Rc::clone(&owner),
    };
    let replaced = CONTRIBUTIONS.with(|all| {
        let mut all = all.borrow_mut();
        if let Some(index) = all.iter().position(|e| same(&e.contribution, &key)) {
            Some(std::mem::replace(&mut all[index], entry))
        } else {
            all.push(entry);
            None
        }
    });
    drop(replaced);
    #[cfg(target_arch = "wasm32")]
    slot::announce();
    ContributionRegistration { key, owner }
}
/// Whether an entry has this package and identifier.
fn same(contribution: &BrowserContribution, key: &(String, String)) -> bool {
    contribution.package == key.0 && contribution.descriptor.id == key.1
}
impl ContributionRegistration {
    /// Remove the contribution while this registration still owns it.
    #[must_use]
    pub fn unregister(self) -> bool {
        let removed = CONTRIBUTIONS.with(|all| {
            let mut all = all.borrow_mut();
            let index = all.iter().position(|e| {
                same(&e.contribution, &self.key) && Rc::ptr_eq(&e.owner, &self.owner)
            })?;
            Some(all.remove(index))
        });
        let removed = removed.is_some();
        #[cfg(target_arch = "wasm32")]
        if removed {
            slot::announce();
        }
        removed
    }
}
/// Contributions attached to `slot`, in registration order.
#[must_use]
pub fn slot_contributions(slot: &str) -> Vec<(BrowserContribution, Rc<dyn ContributionView>)> {
    slot_entries(slot)
        .into_iter()
        .map(|entry| (entry.contribution, entry.view))
        .collect()
}
/// Contributions attached to `slot`, including the identity that owns their mounts.
fn slot_entries(slot: &str) -> Vec<Entry> {
    CONTRIBUTIONS.with(|all| {
        all.borrow()
            .iter()
            .filter(|e| {
                e.contribution.descriptor.target == slot
                    && is_slotted(&e.contribution.descriptor.kind)
            })
            .cloned()
            .collect()
    })
}
/// Kinds mounted by slots rather than renderer maps or the API table.
fn is_slotted(kind: &str) -> bool {
    !matches!(kind, "tool-renderer" | "message-renderer" | "api")
}
/// The page registered for `route`.
/// # Errors
/// Returns a conflict when two contributions claim the route.
pub fn route_contribution(route: &str) -> Result<Option<BrowserContribution>, ContributionFailure> {
    CONTRIBUTIONS.with(|all| {
        let all = all.borrow();
        let mut claims = all
            .iter()
            .filter(|e| e.contribution.descriptor.route.as_deref() == Some(route));
        match (claims.next(), claims.next()) {
            (Some(first), Some(second)) => Err(conflict(route, first, second)),
            (first, _) => Ok(first.map(|e| e.contribution.clone())),
        }
    })
}
/// The input contract the registered contributions declare for `slot`.
/// # Errors
/// Returns a conflict when two contributions declare the slot with different contracts.
pub fn slot_input(slot: &str) -> Result<Option<String>, ContributionFailure> {
    CONTRIBUTIONS.with(|all| {
        let all = all.borrow();
        let mut declared = all.iter().filter_map(|e| {
            let mut slots = e.contribution.descriptor.slots.iter();
            slots.find(|s| s.name == slot).map(|s| (e, &s.input))
        });
        let Some((first, input)) = declared.next() else {
            return Ok(None);
        };
        match declared.find(|(_, other)| *other != input) {
            Some((second, _)) => Err(conflict(slot, first, second)),
            None => Ok(input.clone()),
        }
    })
}
/// The failure for two entries claiming `name`.
fn conflict(name: &str, first: &Entry, second: &Entry) -> ContributionFailure {
    ContributionFailure::Conflict {
        name: name.to_owned(),
        first: first.contribution.descriptor.id.clone(),
        second: second.contribution.descriptor.id.clone(),
    }
}
/// Check a contribution against its slot's contract and the provided APIs.
/// # Errors
/// Returns the first incompatible input or missing required API.
pub fn admit(
    descriptor: &ContributionDescriptor,
    slot_input: Option<&str>,
) -> Result<(), ContributionFailure> {
    if let Some(expected) = &descriptor.input
        && Some(expected.as_str()) != slot_input
    {
        return Err(ContributionFailure::IncompatibleInput {
            id: descriptor.id.clone(),
            expected: expected.clone(),
            supplied: slot_input.map(str::to_owned),
        });
    }
    let missing = PROVIDED_APIS.with(|apis| {
        let apis = apis.borrow();
        descriptor
            .requires
            .iter()
            .find(|name| !apis.contains(name))
            .cloned()
    });
    missing.map_or(Ok(()), |api| {
        Err(ContributionFailure::MissingApi {
            id: descriptor.id.clone(),
            api,
        })
    })
}
/// Record a provided API name; returns false when it was already provided.
#[cfg(target_arch = "wasm32")]
pub(crate) fn provide_api_name(name: &str) -> bool {
    PROVIDED_APIS.with(|apis| {
        let mut apis = apis.borrow_mut();
        let new = !apis.iter().any(|a| a == name);
        if new {
            apis.push(name.to_owned());
        }
        new
    })
}
/// Forget a provided API name.
#[cfg(target_arch = "wasm32")]
pub(crate) fn withdraw_api_name(name: &str) {
    PROVIDED_APIS.with(|apis| apis.borrow_mut().retain(|a| a != name));
}
