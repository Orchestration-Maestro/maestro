//! Ownership and SDK contracts for execution affinity.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, AgentOptions, SharedAgentTool};
use std::{convert::Infallible, sync::Arc};
#[test]
fn default_sdk_owner_and_tools_remain_thread_safe() {
    fn send_sync<T: Send + Sync>() {}
    fn send_future(_: &impl Send) {}
    preserve_sdk_generic_arity::<Infallible>();
    send_sync::<Agent>();
    send_sync::<SharedAgentTool>();
    send_sync::<AgentOptions>();
    let tasks = Arc::new(support::owned::Tasks::default());
    let agent = Arc::new(Agent::new(AgentOptions {
        spawn: Some(tasks.spawner()),
        stream_fn: support::responses(vec![support::assistant("done")]).stream_fn,
        ..Default::default()
    }));
    let observe = agent.prompt(support::owned::input("prompt"));
    send_future(&observe);
    support::run(tasks.take());
    support::run(observe).unwrap();
}

#[test]
fn local_fifo_keeps_owned_items_and_an_independent_result() {
    crate::execution_replay::local::fifo();
    crate::execution_replay::threaded::fifo();
}

/// Keep the ownership witnesses at the public-operation seam.
macro_rules! affinity_scenario {
    ($namespace:ident) => {
        mod $namespace {
            #[test]
            fn owned_run_preserves_hooks_progress_order_and_settlement() {
                crate::execution_replay::$namespace::suspended_owner();
            }
        }
    };
}
affinity_scenario!(local);
affinity_scenario!(threaded);

/// Type-check the existing explicit-message calls without a second type argument.
fn preserve_sdk_generic_arity<C: maestro_agent::CustomAgentMessages + Send + Sync + 'static>() {
    let _ = maestro_agent::agent_loop::<C>;
    let _ = maestro_agent::agent_loop_continue::<C>;
    let _ = maestro_agent::run_agent_loop::<C>;
    let _ = maestro_agent::run_agent_loop_continue::<C>;
    let _ = maestro_agent::agent_loop::local::agent_loop::<C>;
    let _ = maestro_agent::agent_loop::local::agent_loop_continue::<C>;
    let _ = maestro_agent::agent_loop::local::run_agent_loop::<C>;
    let _ = maestro_agent::agent_loop::local::run_agent_loop_continue::<C>;
}
