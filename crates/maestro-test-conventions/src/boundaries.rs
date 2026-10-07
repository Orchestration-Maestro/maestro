use std::collections::BTreeSet;

use crate::source::{Member, Source};
use syn::{ext::IdentExt, parse::Parser, parse::discouraged::Speculative, visit::Visit};

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
            if let Some(syntax) = &source.syntax {
                records.visit_file(syntax);
            } else if let Ok(tokens) = source.contents.parse() {
                records.fragment(tokens);
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

#[derive(Default)]
struct Records {
    declarations: Vec<(String, usize)>,
    inputs: Vec<(usize, proc_macro2::TokenStream)>,
}

impl Records {
    fn declaration(&mut self, name: &syn::Ident, keyword: proc_macro2::Span) {
        self.declarations
            .push((name.unraw().to_string(), keyword.start().line));
    }

    fn fragment(&mut self, tokens: proc_macro2::TokenStream) {
        let parser = |input: syn::parse::ParseStream<'_>| {
            while !input.is_empty() {
                self.fragment_item(input)?;
            }
            Ok(())
        };
        let _ = parser.parse2(tokens);
    }

    fn fragment_item(&mut self, input: syn::parse::ParseStream<'_>) -> syn::Result<()> {
        let fork = input.fork();
        if let Ok(item) = fork.parse::<syn::Item>() {
            input.advance_to(&fork);
            self.visit_item(&item);
            return Ok(());
        }
        if input.fork().parse::<syn::Macro>().is_ok() {
            self.visit_macro(&input.parse()?);
            return Ok(());
        }
        match input.parse::<proc_macro2::TokenTree>()? {
            proc_macro2::TokenTree::Punct(punctuation) if punctuation.as_char() == '#' => {
                if input.peek(syn::Token![!]) {
                    input.parse::<syn::Token![!]>()?;
                }
                if input.peek(syn::token::Bracket) {
                    input.parse::<proc_macro2::Group>()?;
                }
            }
            proc_macro2::TokenTree::Group(group) => self.fragment(group.stream()),
            _ => {}
        }
        Ok(())
    }
}

impl<'ast> Visit<'ast> for Records {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        match item {
            syn::Item::Struct(item) => self.declaration(&item.ident, item.struct_token.span),
            syn::Item::Enum(item) => self.declaration(&item.ident, item.enum_token.span),
            syn::Item::Union(item) => self.declaration(&item.ident, item.union_token.span),
            syn::Item::Trait(item) => self.declaration(&item.ident, item.trait_token.span),
            syn::Item::Type(item) => self.declaration(&item.ident, item.type_token.span),
            syn::Item::Mod(item) => self.declaration(&item.ident, item.mod_token.span),
            _ => {}
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item_type(&mut self, item: &'ast syn::ImplItemType) {
        self.declaration(&item.ident, item.type_token.span);
        syn::visit::visit_impl_item_type(self, item);
    }

    fn visit_trait_item_type(&mut self, item: &'ast syn::TraitItemType) {
        self.declaration(&item.ident, item.type_token.span);
        syn::visit::visit_trait_item_type(self, item);
    }

    fn visit_foreign_item_type(&mut self, item: &'ast syn::ForeignItemType) {
        self.declaration(&item.ident, item.type_token.span);
        syn::visit::visit_foreign_item_type(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let path: Vec<_> = mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.unraw().to_string())
            .collect();
        if path == ["wit_bindgen", "generate"] || path == ["wasmtime", "component", "bindgen"] {
            self.inputs.push((
                mac.path
                    .segments
                    .first()
                    .map_or(1, |segment| segment.ident.span().start().line),
                mac.tokens.clone(),
            ));
        } else {
            self.fragment(mac.tokens.clone());
        }
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
