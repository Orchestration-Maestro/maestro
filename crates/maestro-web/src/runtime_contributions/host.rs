//! The registration interface handed to one module invocation.
use super::{
    BrowserContribution, ContributionFailure, ContributionRegistration,
    adapters::{ElementMessageRenderer, ElementToolRenderer, ElementView},
};
use crate::{
    MessageRenderer,
    page::DomContent,
    tools::types::{ToolRenderer, ToolRendererBinding},
};
use maestro_app::{CustomAgentMessages, JsonObject};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::{JsValue, closure::Closure};

thread_local! {
    /// Provided API implementations by name.
    static API_VALUES: RefCell<HashMap<String, JsValue>> = RefCell::new(HashMap::new());
}
/// A registration this invocation still owns.
pub(super) enum Owned<C: CustomAgentMessages + 'static> {
    /// Tool renderer under a tool name.
    Tool(
        String,
        Rc<dyn ToolRenderer<JsonObject, Value, Box<dyn DomContent>>>,
    ),
    /// Message renderer under a role.
    Message(String, Rc<dyn MessageRenderer<C, Box<dyn DomContent>>>),
    /// Slotted contribution.
    Slot(ContributionRegistration),
    /// Provided API name and the value provided.
    Api(String, JsValue),
}
/// Declarations of one module and what its invocation acquired.
pub(super) struct Invocation<C: CustomAgentMessages + 'static> {
    /// Declared contributions of this module.
    pub(super) declared: Vec<BrowserContribution>,
    /// Acquired registrations, in acquisition order.
    pub(super) owned: Vec<Owned<C>>,
}
/// Native callables backing the interface object.
pub(super) type Callables = Vec<Closure<dyn Fn(JsValue, JsValue) -> Result<JsValue, JsValue>>>;
/// One interface method over the invocation it serves.
type Method<C> = fn(&Rc<RefCell<Invocation<C>>>, &JsValue, &JsValue) -> Result<JsValue, JsValue>;
/// Build the interface object for one invocation.
pub(super) fn interface<C: CustomAgentMessages + serde::Serialize + 'static>(
    invocation: &Rc<RefCell<Invocation<C>>>,
) -> Result<(js_sys::Object, Callables), JsValue> {
    let object = js_sys::Object::new();
    let mut callables: Callables = Vec::new();
    let methods: [(&str, Method<C>); 3] = [
        ("register", register::<C>),
        ("provide", provide::<C>),
        ("api", api::<C>),
    ];
    for (name, method) in methods {
        let state = Rc::clone(invocation);
        let closure: Closure<dyn Fn(JsValue, JsValue) -> Result<JsValue, JsValue>> =
            Closure::new(move |first, second| method(&state, &first, &second));
        js_sys::Reflect::set(&object, &name.into(), closure.as_ref())?;
        callables.push(closure);
    }
    Ok((object, callables))
}
/// A string argument of an interface method, or the thrown refusal naming it.
fn string(value: &JsValue, method: &str, argument: &str) -> Result<String, JsValue> {
    value
        .as_string()
        .ok_or_else(|| JsValue::from_str(&format!("{method}: {argument} is not a string")))
}
/// Find the declaration for a registered identifier.
fn declared<C: CustomAgentMessages>(
    invocation: &Rc<RefCell<Invocation<C>>>,
    id: &str,
) -> Result<BrowserContribution, JsValue> {
    invocation
        .borrow()
        .declared
        .iter()
        .find(|c| c.descriptor.id == id)
        .cloned()
        .ok_or_else(|| undeclared(id.to_owned()))
}
/// The thrown value for an identifier the module did not declare with this kind.
fn undeclared(id: String) -> JsValue {
    JsValue::from_str(&ContributionFailure::Undeclared { id }.to_string())
}
/// Bind a declared contribution to an element tag the module defined.
fn register<C: CustomAgentMessages + serde::Serialize + 'static>(
    invocation: &Rc<RefCell<Invocation<C>>>,
    id: &JsValue,
    tag: &JsValue,
) -> Result<JsValue, JsValue> {
    let id = string(id, "register", "identifier")?;
    let tag = string(tag, "register", "tag")?;
    let contribution = declared(invocation, &id)?;
    if contribution.descriptor.kind == "api" {
        return Err(undeclared(contribution.descriptor.id));
    }
    let target = contribution.descriptor.target.clone();
    let owned = match contribution.descriptor.kind.as_str() {
        "tool-renderer" => {
            let renderer: Rc<dyn ToolRenderer<JsonObject, Value, Box<dyn DomContent>>> =
                Rc::new(ElementToolRenderer {
                    tag,
                    descriptor: contribution.descriptor,
                });
            crate::register_tool_renderer(
                &target,
                ToolRendererBinding::from_dom(Rc::clone(&renderer)),
            );
            Owned::Tool(target, renderer)
        }
        "message-renderer" => {
            let renderer: Rc<dyn MessageRenderer<C, Box<dyn DomContent>>> =
                Rc::new(ElementMessageRenderer {
                    tag,
                    descriptor: contribution.descriptor,
                });
            crate::register_message_renderer(&target, Rc::clone(&renderer));
            Owned::Message(target, renderer)
        }
        _ => Owned::Slot(super::register_contribution(
            contribution,
            Rc::new(ElementView { tag }),
        )),
    };
    invocation.borrow_mut().owned.push(owned);
    Ok(JsValue::UNDEFINED)
}
/// Provide a declared API implementation.
fn provide<C: CustomAgentMessages + 'static>(
    invocation: &Rc<RefCell<Invocation<C>>>,
    id: &JsValue,
    value: &JsValue,
) -> Result<JsValue, JsValue> {
    let contribution = declared(invocation, &string(id, "provide", "identifier")?)?;
    if contribution.descriptor.kind != "api" {
        return Err(undeclared(contribution.descriptor.id));
    }
    let name = contribution.descriptor.target;
    if !super::provide_api_name(&name) {
        return Err(JsValue::from_str(&format!(
            "{name}: host API already provided"
        )));
    }
    API_VALUES.with(|all| all.borrow_mut().insert(name.clone(), value.clone()));
    invocation
        .borrow_mut()
        .owned
        .push(Owned::Api(name, value.clone()));
    Ok(JsValue::UNDEFINED)
}
/// Resolve an API this module declared as required.
fn api<C: CustomAgentMessages + 'static>(
    invocation: &Rc<RefCell<Invocation<C>>>,
    name: &JsValue,
    _: &JsValue,
) -> Result<JsValue, JsValue> {
    let api = string(name, "api", "name")?;
    let requiring = invocation
        .borrow()
        .declared
        .iter()
        .find(|c| c.descriptor.requires.contains(&api))
        .map(|c| c.descriptor.id.clone());
    let Some(id) = requiring else {
        return Err(undeclared(api));
    };
    API_VALUES
        .with(|all| all.borrow().get(&api).cloned())
        .ok_or_else(|| JsValue::from_str(&ContributionFailure::MissingApi { id, api }.to_string()))
}
/// Release each registration the invocation still owns, newest first.
pub(super) fn release<C: CustomAgentMessages + 'static>(owned: Vec<Owned<C>>) {
    for registration in owned.into_iter().rev() {
        match registration {
            // A later replacement owns the entry when a guarded removal returns false.
            Owned::Tool(name, renderer) => {
                crate::tools::renderer_registry::unregister_tool_renderer(&name, &renderer);
            }
            Owned::Message(role, renderer) => {
                crate::components::message_renderer_registry::unregister_message_renderer(
                    &role, &renderer,
                );
            }
            Owned::Slot(registration) => {
                let _ = registration.unregister();
            }
            Owned::Api(name, value) => {
                let removed = API_VALUES.with(|all| {
                    let mut all = all.borrow_mut();
                    let owned = all.get(&name).is_some_and(|current| *current == value);
                    owned.then(|| all.remove(&name)).flatten()
                });
                if removed.is_some() {
                    super::withdraw_api_name(&name);
                }
            }
        }
    }
}
