//! Public discovery-result formatting without native effects.
#![cfg(test)]
use crate::agents::{AgentConfig, format_agent_list};

/// Supplies literal authored metadata to the public formatting seam.
fn agent(name: &str, source: &str) -> AgentConfig {
    AgentConfig {
        name: name.into(),
        description: format!("{name} description"),
        tools: None,
        model: None,
        system_prompt: String::new(),
        source: source.into(),
        file_path: format!("/{name}.md"),
    }
}

#[test]
fn agent_lists_keep_order_and_report_unlisted_definitions() {
    assert_eq!(format_agent_list(&[], 2), ("none".into(), 0));
    let agents = [agent("one", "user"), agent("two", "project")];
    assert_eq!(
        format_agent_list(&agents, 1),
        ("one (user): one description".into(), 1)
    );
    assert_eq!(format_agent_list(&agents, 0), (String::new(), 2));
    assert_eq!(
        format_agent_list(&agents, 10),
        (
            "one (user): one description; two (project): two description".into(),
            0
        )
    );
}
