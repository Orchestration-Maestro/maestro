//! Right-click pass-through modifier selection.

use maestro_protocol::frames::KeyModifiers;
use serde::{Deserialize, de};

/// Optional shared modifier bits that pass right-click gestures to pane applications.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RightClickPassthroughModifierConfig(Option<u8>);

impl RightClickPassthroughModifierConfig {
    /// Required modifier bits, or no pass-through binding.
    #[must_use]
    pub const fn modifiers(self) -> Option<u8> {
        self.0
    }
}

impl<'de> Deserialize<'de> for RightClickPassthroughModifierConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        parse_right_click_passthrough_modifier(&value).ok_or_else(|| {
            de::Error::custom("multiplexer.rightClickPassthroughModifier must be empty, off, none, disabled, ctrl/control, alt/option, cmd/command/super, meta, hyper, or a + separated combination without shift")
        })
    }
}

/// Parse disabled spellings or a nonempty combination without shift.
fn parse_right_click_passthrough_modifier(
    value: &str,
) -> Option<RightClickPassthroughModifierConfig> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || ["off", "none", "disabled"]
            .iter()
            .any(|word| trimmed.eq_ignore_ascii_case(word))
    {
        return Some(RightClickPassthroughModifierConfig(None));
    }
    let mut modifiers = KeyModifiers::NONE;
    for token in trimmed.split('+') {
        modifiers |= match token.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "alt" | "option" => KeyModifiers::ALT,
            "cmd" | "command" | "super" => KeyModifiers::SUPER,
            "meta" => KeyModifiers::META,
            "hyper" => KeyModifiers::HYPER,
            _ => return None,
        };
    }
    (modifiers != KeyModifiers::NONE)
        .then_some(RightClickPassthroughModifierConfig(Some(modifiers)))
}
