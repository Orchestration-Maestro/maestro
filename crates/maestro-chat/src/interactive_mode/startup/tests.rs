//! The startup help text's two renderings.
use super::ExpandableText;
use crate::interactive_mode::tests::{Expected, FIXTURES, Input, cases};
use maestro_tui::Component;

#[test]
fn expandable_startup_text_replaces_both_rendered_variants() {
    for (input, expected) in cases::<Input, Expected>(
        FIXTURES,
        "expandable_startup_text_replaces_both_rendered_variants",
    ) {
        let (
            Input::Expandable {
                collapsed,
                expanded,
                initial,
                width,
            },
            Expected::Expandable { before, after },
        ) = (input, expected)
        else {
            panic!("an expandable case");
        };
        let text = ExpandableText::new(
            Box::new(move || Ok(collapsed.clone())),
            Box::new(move || Ok(expanded.clone())),
            initial,
            1,
            0,
        )
        .unwrap();
        assert_eq!(text.render(width), before);
        text.set_expanded(!initial).unwrap();
        assert_eq!(text.render(width), after);
    }
}
