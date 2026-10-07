use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use syn::{
    Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

#[derive(Default)]
pub(super) struct WitInputs {
    guest: BTreeSet<PathBuf>,
    host: BTreeSet<PathBuf>,
    guest_declared: bool,
    host_declared: bool,
    host_location: Option<String>,
}

impl WitInputs {
    pub(super) fn scan(
        &mut self,
        context: &ScanContext<'_>,
        line: usize,
        tokens: proc_macro2::TokenStream,
    ) -> Result<(), String> {
        let location = format!("{}:{line}", context.path.display());
        let error = || {
            format!("{location}: WIT code-generation input requires review: expected static path")
        };
        let input = syn::parse2::<Inputs>(tokens).map_err(|_| error())?;
        if input.paths.is_empty() && !input.inline {
            return Err(error());
        }
        let is_guest = context.owner == "maestro-extensions-wasm";
        if input.inline && !is_guest {
            return Err(format!("{location}: inline WIT must be guest-owned"));
        }
        if input.inline && context.guest.is_none() {
            return Err(format!(
                "{location}: WIT input has no guest-owned source member"
            ));
        }
        if is_guest {
            self.guest_declared = true;
        } else {
            self.host_declared = true;
            self.host_location = Some(location.clone());
        }
        for path in input.paths {
            let root = context.canonical_input(&path, &location)?;
            if is_guest {
                self.guest.insert(root);
            } else {
                self.host.insert(root);
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), String> {
        if self.guest_declared && self.host_declared && self.guest != self.host {
            return Err(format!(
                "{}: host and guest WIT inputs must share the same canonical source roots",
                self.host_location.as_deref().unwrap_or("WIT")
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Inputs {
    paths: Vec<String>,
    inline: bool,
}

impl Parse for Inputs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            input.parse::<syn::LitStr>()?;
            return Ok(Self {
                paths: vec!["wit".into()],
                inline: false,
            });
        }
        if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            fields(&content)
        } else {
            fields(input)
        }
    }
}

fn fields(input: ParseStream<'_>) -> syn::Result<Inputs> {
    let mut result = Inputs::default();
    while !input.is_empty() {
        let name: syn::Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        if name == "path" {
            result.paths.extend(paths(input)?);
        } else if name == "inline" {
            input.parse::<syn::LitStr>()?;
            result.inline = true;
        } else {
            skip_value(input)?;
        }
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(result)
}

fn paths(input: ParseStream<'_>) -> syn::Result<Vec<String>> {
    if input.peek(syn::LitStr) {
        return Ok(vec![input.parse::<syn::LitStr>()?.value()]);
    }
    let content;
    syn::bracketed!(content in input);
    Punctuated::<syn::LitStr, Token![,]>::parse_terminated(&content)
        .map(|paths| paths.iter().map(syn::LitStr::value).collect())
}

fn skip_value(input: ParseStream<'_>) -> syn::Result<()> {
    while !input.is_empty() && !input.peek(Token![,]) {
        input.parse::<proc_macro2::TokenTree>()?;
    }
    Ok(())
}

pub(super) struct ScanContext<'a> {
    pub(super) owner: &'a str,
    pub(super) directory: &'a Path,
    pub(super) guest: Option<&'a Path>,
    pub(super) path: &'a Path,
}

impl ScanContext<'_> {
    fn canonical_input(&self, input: &str, location: &str) -> Result<PathBuf, String> {
        let input = self
            .directory
            .join(input)
            .canonicalize()
            .map_err(|error| format!("{location}: cannot resolve WIT input: {error}"))?;
        let guest = self
            .guest
            .ok_or_else(|| format!("{location}: WIT input has no guest-owned source member"))?;
        if !input.starts_with(guest) {
            return Err(format!(
                "{location}: WIT input must use the guest-owned canonical source: {}",
                input.display()
            ));
        }
        if input.is_dir() {
            Ok(input)
        } else {
            input.parent().map(Path::to_path_buf).ok_or_else(|| {
                format!(
                    "{location}: WIT code-generation input requires review: expected static path"
                )
            })
        }
    }
}
