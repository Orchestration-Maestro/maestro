//! Sidebar rows, identity, bounds and occurrence styles.

use super::super::navigation::{AgentSidebarToken, SidebarConfig, SpaceSidebarToken};
use maestro_detection::agents::{Agent, agent_label};
use serde_json::{Value, json};

fn decode(value: Value) -> SidebarConfig {
    serde_json::from_value(value).unwrap()
}

#[test]
fn sidebar_defaults_keep_row_order() {
    let config = decode(json!({}));
    assert_eq!(
        serde_json::to_value(&config).unwrap(),
        json!({
            "agents": {"rows": [["state_icon", "workspace", "tab"], ["agent"]],
                       "rowsByAgent": {}, "rowGap": 0},
            "spaces": {"rows": [["state_icon", "workspace"], ["branch", "git_status"]], "rowGap": 0}
        })
    );
}

#[test]
fn sidebar_tokens_keep_custom_identity() {
    let input = json!({"agents": {
        "rows": [["state_icon", "workspace", "tab", "pane"],
                 ["state_text", "agent", "$summary"],
                 ["terminal_title", "terminal_title_stripped", "$terminal_title"]],
        "rowsByAgent": {"claude": [["terminal_title_stripped"], ["agent", "$model"]]},
        "rowGap": 1}, "spaces": {"rows": [["workspace"], ["$jj_status"]], "rowGap": 3}});
    let config = decode(input.clone());
    assert_eq!(
        config.agents.rows[2][2],
        AgentSidebarToken::Custom("terminal_title".into())
    );
    assert_eq!(
        config.spaces.rows[1][0],
        SpaceSidebarToken::Custom("jj_status".into())
    );
    assert_eq!(
        config.agents.rows_for_agent(Some(Agent::Claude)),
        &config.agents.rows_by_agent["claude"]
    );
    assert_eq!(
        config.agents.rows_for_agent(Some(Agent::Codex)),
        &config.agents.rows
    );
    assert_eq!(config.agents.rows_for_agent(None), &config.agents.rows);
    assert_eq!(serde_json::to_value(&config).unwrap(), input);
    assert_eq!(
        serde_json::to_value(AgentSidebarToken::from("workspace".to_owned())).unwrap(),
        "$workspace"
    );
    assert_eq!(
        serde_json::to_value(SpaceSidebarToken::from("branch".to_owned())).unwrap(),
        "$branch"
    );
}

/// Checks occurrence identity and one-layer projection for both token families.
macro_rules! assert_projection {
    ($name:ident, $plain:expr, $family:ident) => {
        /// Checks the token family through its public projection method.
        fn $name() {
            let plain = $plain;
            assert!(std::ptr::eq(plain.parts().0, &plain));
            assert_eq!(plain.parts().1, Default::default());
            let style = super::super::navigation::SidebarTokenStyle {
                bold: Some(false),
                dim: Some(true),
                ..Default::default()
            };
            let styled = $family::Styled {
                token: Box::new(plain),
                style,
            };
            let $family::Styled { token, .. } = &styled else {
                unreachable!()
            };
            assert!(std::ptr::eq(styled.parts().0, token.as_ref()));
            assert_eq!(styled.parts().1, style);
            let nested = $family::Styled {
                token: Box::new(styled),
                style: Default::default(),
            };
            let $family::Styled { token, .. } = &nested else {
                unreachable!()
            };
            assert!(std::ptr::eq(nested.parts().0, token.as_ref()));
            assert!(matches!(nested.parts().0, $family::Styled { .. }));
            assert_eq!(nested.parts().1, Default::default());
        }
    };
}

assert_projection!(
    assert_agent_projection,
    AgentSidebarToken::Workspace,
    AgentSidebarToken
);
assert_projection!(
    assert_space_projection,
    SpaceSidebarToken::GitStatus,
    SpaceSidebarToken
);

#[test]
fn sidebar_styles_belong_to_each_occurrence() {
    let config = decode(json!({"agents": {"rows": [[
        {"token": "workspace", "fg": "#aBc", "bold": false}, "workspace"],
        [{"token": "$summary", "dim": false}]],
        "rowsByAgent": {"claude": [[{"token": "agent", "fg": "#112233", "bold": true, "dim": false}]]}},
        "spaces": {"rows": [[{"token": "git_status", "fg": "#ff00aa"}], [{"token": "$jj", "bold": true}]]}}));
    let (token, style) = config.agents.rows[0][0].parts();
    assert_eq!(token, &AgentSidebarToken::Workspace);
    assert_eq!(style.bold, Some(false));
    assert_eq!(serde_json::to_value(style.fg.unwrap()).unwrap(), "#aabbcc");
    assert_eq!(config.agents.rows[0][1], AgentSidebarToken::Workspace);
    let encoded = serde_json::to_value(&config).unwrap();
    assert_eq!(
        encoded["agents"]["rows"][0][0],
        json!({"token": "workspace", "fg": "#aabbcc", "bold": false})
    );
    assert_eq!(
        encoded["agents"]["rows"][1][0],
        json!({"token": "$summary", "dim": false})
    );
    assert_eq!(
        encoded["agents"]["rowsByAgent"]["claude"][0][0],
        json!({"token": "agent", "fg": "#112233", "bold": true, "dim": false})
    );
    let (token, style) = config.spaces.rows[0][0].parts();
    assert_eq!(token, &SpaceSidebarToken::GitStatus);
    assert_eq!(serde_json::to_value(style.fg.unwrap()).unwrap(), "#ff00aa");
    assert_eq!(config.spaces.rows[1][0].parts().1.bold, Some(true));
    assert_eq!(decode(encoded), config);
    assert_agent_projection();
    assert_space_projection();
}

#[test]
fn sidebar_rejects_malformed_styles() {
    for entry in [
        json!(["workspace", null, null, null]),
        json!({"token":"workspace", "fg":"red"}),
        json!({"token":"workspace", "fg":"#abcd"}),
        json!({"token":"workspace", "underline":true}),
        json!({"token":"workspace", "bold":"false"}),
        json!({"token":"workspace", "fg":"#éab"}),
    ] {
        assert!(
            serde_json::from_value::<SidebarConfig>(json!({"agents":{"rows":[[entry]]}})).is_err()
        );
    }
}

#[test]
fn sidebar_rejects_bad_custom_token_names() {
    for token in [
        "summary",
        "$",
        "$bad.name",
        "$a b",
        &format!("${}", "a".repeat(33)),
    ] {
        assert!(
            serde_json::from_value::<SidebarConfig>(json!({"agents":{"rows":[[token]]}})).is_err()
        );
    }
    let longest = format!("${}", "a".repeat(32));
    decode(json!({"agents":{"rows":[[longest, "$_-09"]]}}));
}

#[test]
fn sidebar_limits_cover_rows_tokens_and_overrides() {
    for count in [16, 17] {
        let rows = vec![vec!["workspace"]; count];
        let tokens = vec!["workspace"; count];
        for value in [
            json!({"agents":{"rows":rows}}),
            json!({"spaces":{"rows":rows}}),
            json!({"agents":{"rowsByAgent":{"claude":rows}}}),
            json!({"agents":{"rows":[tokens]}}),
            json!({"spaces":{"rows":[tokens]}}),
            json!({"agents":{"rowsByAgent":{"claude":[tokens]}}}),
        ] {
            assert_eq!(
                serde_json::from_value::<SidebarConfig>(value).is_ok(),
                count == 16
            );
        }
    }
}

#[test]
fn sidebar_accepts_canonical_agent_keys() {
    let overrides: serde_json::Map<String, Value> = Agent::ALL
        .into_iter()
        .map(|agent| {
            (
                agent_label(agent).into(),
                json!([["agent", format!("${}", agent_label(agent))]]),
            )
        })
        .collect();
    let config = decode(json!({"agents":{"rowsByAgent":overrides}}));
    assert_eq!(config.agents.rows_by_agent.len(), Agent::ALL.len());
    for agent in Agent::ALL {
        assert_eq!(
            config.agents.rows_for_agent(Some(agent))[0][1],
            AgentSidebarToken::Custom(agent_label(agent).into())
        );
    }
}

#[test]
fn sidebar_rejects_agent_alias_keys() {
    for key in ["claude-code", "Claude", " claude ", "unknown"] {
        assert!(
            serde_json::from_value::<SidebarConfig>(
                json!({"agents":{"rowsByAgent":{key:[["agent"]]}}})
            )
            .is_err()
        );
    }
}
