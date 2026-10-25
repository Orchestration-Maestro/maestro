//! Captured model labels and their single-walk work bound.
use super::model_list;
use maestro_app::{Model, ScopedModel};
use std::{
    cell::Cell,
    sync::{Arc, RwLock},
};

/// An independently labeled catalog descriptor.
fn model(id: &str, thinking: Option<&str>) -> ScopedModel {
    let descriptor: Model = serde_json::from_value(serde_json::json!({
        "id": id, "name": "model", "api": "openai-completions", "provider": "test",
        "baseUrl": "https://example.test", "reasoning": true, "input": ["text"],
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
        "contextWindow": 1000, "maxTokens": 100
    }))
    .unwrap();
    ScopedModel {
        model: Arc::new(RwLock::new(descriptor)),
        thinking_level: thinking.map(|level| level.parse().unwrap()),
    }
}

#[test]
fn scope_formatting_has_linear_work() {
    let projections: Vec<(Vec<ScopedModel>, &str)> = vec![
        (vec![], ""),
        (vec![model("alpha", None)], "alpha"),
        (
            vec![model("alpha", Some("off")), model("beta", Some("high"))],
            "alpha:off, beta:high",
        ),
        (
            vec![
                model("a,b", None),
                model("a,b", None),
                model("λ:中", Some("low")),
            ],
            "a,b, a,b, λ:中:low",
        ),
        (
            vec![model("", None), model("literal\nname", Some("minimal"))],
            ", literal\nname:minimal",
        ),
        (
            ["off", "minimal", "low", "medium", "high", "xhigh"]
                .map(|level| model("same", Some(level)))
                .into(),
            "same:off, same:minimal, same:low, same:medium, same:high, same:xhigh",
        ),
    ];
    for (models, expected) in projections {
        assert_eq!(model_list(&models), expected);
    }
    for n in [64, 128, 256] {
        let models: Vec<_> = (0..n).map(|_| model("a", None)).collect();
        let visits = Cell::new(0);
        let actual = model_list(models.iter().inspect(|_| visits.set(visits.get() + 1)));
        assert_eq!(actual, vec!["a"; n].join(", "));
        assert!(visits.get() <= n, "single-walk upper bound");
        let linear_control = models.iter().fold(0, |count, _| count + 1);
        assert_eq!(linear_control, n);
        let rescan_negative: usize = models
            .iter()
            .enumerate()
            .map(|(i, _)| models[i..].iter().fold(0, |count, _| count + 1))
            .sum();
        assert!(rescan_negative > n, "repeated-suffix negative control");
    }
}
