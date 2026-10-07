use std::collections::BTreeSet;

use crate::source::{Member, Source};
use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;

mod wit;
use wit::{ScanContext, WitInputs};

pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let guest = members
        .iter()
        .find(|member| member.name == "maestro-extensions-wasm")
        .map(|member| {
            member
                .directory
                .canonicalize()
                .map_err(|error| format!("cannot resolve guest member: {error}"))
        })
        .transpose()?;
    let mut declarations = BTreeSet::new();
    let mut wit = WitInputs::default();
    for member in members {
        for source in &member.sources {
            let mut records = Records::default();
            if let Ok(tokens) = source.contents.parse() {
                records.scan(tokens);
            }
            check_declarations(member, source, &records, &mut declarations)?;
            let context = ScanContext {
                owner: &member.name,
                directory: &member.directory,
                guest: guest.as_deref(),
                path: &source.path,
            };
            for (line, tokens) in records.inputs {
                wit.scan(&context, line, tokens)?;
            }
        }
    }
    wit.finish()
}

const DECLARATION_KINDS: &[&str] = &["struct", "enum", "union", "trait", "type", "mod"];

#[derive(Default)]
struct Records {
    declarations: Vec<(String, usize)>,
    inputs: Vec<(usize, proc_macro2::TokenStream)>,
}

impl Records {
    fn scan(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            let generated = matches!(tokens[..index].last(), Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '$');
            match token {
                TokenTree::Ident(keyword) if !generated => {
                    self.declaration(keyword, &tokens[index + 1..]);
                    self.wit_macro(&tokens[index..]);
                }
                TokenTree::Group(group) if !attribute_group(&tokens[..index]) => {
                    self.scan(group.stream());
                }
                _ => {}
            }
        }
    }

    fn declaration(&mut self, keyword: &proc_macro2::Ident, following: &[TokenTree]) {
        let kind = keyword.to_string();
        if !DECLARATION_KINDS.contains(&kind.as_str()) {
            return;
        }
        let Some(TokenTree::Ident(name)) = following.first() else {
            return;
        };
        if kind == "union" && !union_body(following.get(1)) {
            return;
        }
        self.declarations
            .push((name.unraw().to_string(), keyword.span().start().line));
    }

    fn wit_macro(&mut self, tokens: &[TokenTree]) {
        if !matches!(tokens.first(), Some(TokenTree::Ident(name)) if name == "wit_bindgen" || name == "wasmtime")
        {
            return;
        }
        let Some(end) = tokens
            .iter()
            .position(|token| matches!(token, TokenTree::Group(_)))
        else {
            return;
        };
        let Ok(mac) = syn::parse2::<syn::Macro>(tokens[..=end].iter().cloned().collect()) else {
            return;
        };
        let path: Vec<_> = mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.unraw().to_string())
            .collect();
        if path == ["wit_bindgen", "generate"] || path == ["wasmtime", "component", "bindgen"] {
            self.inputs
                .push((tokens[0].span().start().line, mac.tokens));
        }
    }
}

fn attribute_group(previous: &[TokenTree]) -> bool {
    match previous {
        [.., TokenTree::Punct(hash)] if hash.as_char() == '#' => true,
        [.., TokenTree::Punct(hash), TokenTree::Punct(bang)] => {
            hash.as_char() == '#' && bang.as_char() == '!'
        }
        _ => false,
    }
}

fn union_body(token: Option<&TokenTree>) -> bool {
    match token {
        Some(TokenTree::Group(group)) => group.delimiter() == proc_macro2::Delimiter::Brace,
        Some(TokenTree::Punct(punctuation)) => punctuation.as_char() == '<',
        Some(TokenTree::Ident(keyword)) => keyword == "where",
        _ => false,
    }
}

fn selector(name: &str) -> bool {
    matches!(
        name,
        "ConfigSelectorComponent"
            | "ExtensionSelectorComponent"
            | "ModelSelectorComponent"
            | "OAuthSelectorComponent"
            | "ScopedModelsSelectorComponent"
            | "SessionSelectorComponent"
            | "SettingsSelectorComponent"
            | "ShowImagesSelectorComponent"
            | "ThemeSelectorComponent"
            | "ThinkingSelectorComponent"
            | "TreeSelectorComponent"
            | "UserMessageSelectorComponent"
            | "config_selector"
            | "extension_selector"
            | "model_selector"
            | "oauth_selector"
            | "scoped_models_selector"
            | "session_selector"
            | "settings_selector"
            | "show_images_selector"
            | "theme_selector"
            | "thinking_selector"
            | "tree_selector"
            | "user_message_selector"
            | "session_selector_search"
    )
}

fn check_declarations(
    member: &Member,
    source: &Source,
    records: &Records,
    declarations: &mut BTreeSet<String>,
) -> Result<(), String> {
    for (name, line) in &records.declarations {
        let owner = if selector(name) {
            "maestro-chat"
        } else if matches!(
            name.as_str(),
            "ToolDefinition" | "ToolRenderContext" | "ToolRenderResultOptions"
        ) {
            "maestro-tools"
        } else {
            continue;
        };
        if member.name != owner {
            return Err(format!(
                "{}:{line}: {name} declaration belongs to {owner}, not {}",
                source.path.display(),
                member.name
            ));
        }
        if owner == "maestro-tools" && !declarations.insert(name.clone()) {
            return Err(format!(
                "{}:{line}: duplicate declaration {name}",
                source.path.display()
            ));
        }
    }
    Ok(())
}
