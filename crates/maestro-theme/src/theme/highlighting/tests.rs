//! Native scope classification and grammar identity.
use super::{SyntectHighlighter, rule_color, scope_color};
use serde_json::Value;
use syntect::parsing::Scope;

/// The key name a scope maps to.
fn key_of(scope: &str) -> Option<String> {
    rule_color(scope).map(|color| color.as_str().to_owned())
}

/// The key name the innermost mapped scope of a stack maps to.
fn stack_key(scopes: &[&str]) -> Option<String> {
    let stack: Vec<Scope> = scopes.iter().map(|s| Scope::new(s).unwrap()).collect();
    scope_color(&stack).map(|color| color.as_str().to_owned())
}

#[test]
fn syntax_scopes_map_only_the_nine_theme_keys() {
    let cases = [
        ("comment.line.double-slash.js", "syntaxComment"),
        ("keyword.control.conditional.js", "syntaxKeyword"),
        ("keyword", "syntaxKeyword"),
        ("entity.name.function.js", "syntaxFunction"),
        ("support.function.console.js", "syntaxFunction"),
        ("variable.other.readwrite.js", "syntaxVariable"),
        ("entity.other.attribute-name.html", "syntaxVariable"),
        ("string.quoted.double.python", "syntaxString"),
        ("constant.numeric.integer.decimal.js", "syntaxNumber"),
        ("constant.language.boolean.true.js", "syntaxNumber"),
        ("storage.type.js", "syntaxType"),
        ("entity.name.type.ts", "syntaxType"),
        ("entity.name.class.js", "syntaxType"),
        ("support.class.js", "syntaxType"),
        ("keyword.operator.assignment.js", "syntaxOperator"),
        ("punctuation.terminator.statement.js", "syntaxPunctuation"),
    ];
    for (scope, key) in cases {
        assert_eq!(key_of(scope).as_deref(), Some(key), "{scope}");
    }
    let mut reached: Vec<_> = cases.iter().map(|(_, key)| *key).collect();
    reached.sort_unstable();
    reached.dedup();
    assert_eq!(reached.len(), 9);
    for unmapped in [
        "source.js",
        "meta.function-call.js",
        "constant.character.escape.js",
        "storage.modifier.js",
        "keywords.other",
        "commentary",
    ] {
        assert_eq!(key_of(unmapped), None, "{unmapped}");
    }
}

#[test]
fn syntax_scopes_choose_the_innermost_mapped_scope() {
    let stack = |scopes: &[&str]| stack_key(scopes);
    assert_eq!(
        stack(&["source.js", "comment.line", "meta.x"]).as_deref(),
        Some("syntaxComment")
    );
    assert_eq!(
        stack(&["comment.block", "punctuation.definition.comment"]).as_deref(),
        Some("syntaxPunctuation")
    );
    assert_eq!(
        stack(&["string.quoted", "variable.other.interpolated"]).as_deref(),
        Some("syntaxVariable")
    );
    assert_eq!(
        stack(&["keyword.control", "keyword.operator.logical"]).as_deref(),
        Some("syntaxOperator")
    );
    assert_eq!(stack(&["source.js", "meta.x"]), None);
    assert_eq!(stack(&[]), None);
}

#[test]
fn language_aliases_keep_the_selected_grammar_identity() {
    let highlighter = SyntectHighlighter::new().unwrap();
    let entries: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/language_grammars.json"
    ))
    .unwrap();
    let mut selected = 0;
    for entry in &entries {
        let (token, native) = (entry["token"].as_str().unwrap(), &entry["native"]);
        let Some(native) = native.as_str() else {
            continue;
        };
        for variant in [token.to_owned(), token.to_ascii_uppercase()] {
            let grammar = highlighter
                .find(&variant)
                .map(|syntax| syntax.name.as_str());
            assert_eq!(grammar, Some(native), "{variant}");
        }
        selected += 1;
    }
    assert_eq!(selected, 190);
}
