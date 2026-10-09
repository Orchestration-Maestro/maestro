#![doc = include_str!("../../../../docs/models/oauth.md")]
pub mod device;
pub mod oauth_page;
pub mod pkce;
pub mod responses;
pub mod subscription;
pub mod types;

mod callback;
mod native;
pub use device::github_copilot::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};

/// Trim the authorization whitespace set without stripping other format characters.
pub(crate) fn authorization_whitespace(character: char) -> bool {
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
