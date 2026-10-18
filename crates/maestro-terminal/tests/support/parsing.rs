//! Lossless projections of terminal command records.

use maestro_terminal::{
    color::Rgb,
    kitty,
    osc::{
        self, Command,
        color::{Request, Target},
        semantic_prompt,
    },
};
use serde_json::{Value, json};

/// Protocol enum spelling from Rust's source-named variant.
pub(crate) fn wire_name(value: impl std::fmt::Debug) -> String {
    let mut out = String::new();
    for (i, c) in format!("{value:?}").chars().enumerate() {
        if c.is_ascii_uppercase() && i != 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// All RGB fields in their source representation.
pub(crate) fn rgb(v: Rgb) -> Value {
    json!({"r":v.r,"g":v.g,"b":v.b})
}

/// Preserve palette, special and dynamic target identity.
fn target(v: Target) -> Value {
    match v {
        Target::Palette(n) => json!({"palette":n}),
        Target::Special(s) => json!({"special":wire_name(s)}),
        Target::Dynamic(d) => json!({"dynamic":wire_name(d)}),
    }
}

/// Preserve one colour request with all payload fields.
fn request(v: &Request) -> Value {
    match v {
        Request::Set(v) => json!({"set":{"target":target(v.target),"color":rgb(v.color)}}),
        Request::Query(v) => json!({"query":target(*v)}),
        Request::Reset(v) => json!({"reset":target(*v)}),
        Request::ResetPalette => json!({"reset_palette":{}}),
        Request::ResetSpecial => json!({"reset_special":{}}),
    }
}

/// Preserve the colour-protocol key namespace.
fn key(v: kitty::color::Kind) -> Value {
    match v {
        kitty::color::Kind::Palette(n) => json!({"palette":n}),
        kitty::color::Kind::Special(s) => json!({"special":wire_name(s)}),
    }
}

/// Preserve the complete ordered colour-protocol request.
fn kitty_request(v: kitty::color::Request) -> Value {
    use kitty::color::Request as K;
    match v {
        K::Set { key: k, color } => json!({"set":{"key":key(k),"color":rgb(color)}}),
        K::Query(v) => json!({"query":key(v)}),
        K::Reset(v) => json!({"reset":key(v)}),
    }
}

/// Read a prompt option without defaulting an absent or invalid value.
fn prompt_option(v: Option<semantic_prompt::OptionValue>) -> Value {
    match v {
        Some(semantic_prompt::OptionValue::PromptKind(k)) => json!(wire_name(k)),
        Some(semantic_prompt::OptionValue::Redraw(r)) => json!(wire_name(r)),
        None => Value::Null,
    }
}

/// Preserve command variant, raw bytes, duplicates and request ordering.
pub(crate) fn command(v: Command) -> Value {
    match v {
        Command::ChangeWindowTitle(v) => json!({"change_window_title":v}),
        Command::ClipboardContents { kind, data } => {
            json!({"clipboard_contents":{"kind":kind,"data":data}})
        }
        Command::ReportPwd { value } => json!({"report_pwd":{"value":value}}),
        Command::HyperlinkStart { id, uri } => json!({"hyperlink_start":{"id":id,"uri":uri}}),
        Command::HyperlinkEnd => json!({"hyperlink_end":{}}),
        Command::SemanticPrompt(v) => json!({"semantic_prompt":{
            "action":wire_name(v.action),"options":v.options_unvalidated,
            "prompt_kind":prompt_option(v.read_option(semantic_prompt::Option::PromptKind)),
            "redraw":prompt_option(v.read_option(semantic_prompt::Option::Redraw)),
        }}),
        Command::ColorOperation {
            op,
            requests,
            terminator,
        } => json!({"color_operation":{
            "op":format!("osc_{}", op as u16),
            "requests":requests.iter().map(request).collect::<Vec<_>>(),
            "terminator":wire_name(terminator),
        }}),
        Command::KittyColorProtocol(v) => json!({"kitty_color_protocol":{
            "list":v.list.into_iter().map(kitty_request).collect::<Vec<_>>(),
            "terminator":wire_name(v.terminator),
        }}),
    }
}

/// Exact terminator bytes from the public decoder.
pub(crate) fn terminator_bytes(term: Option<u8>) -> &'static [u8] {
    osc::Terminator::init(term).string()
}

/// Preserve a VT action's complete payload at the source byte and slot.
pub(crate) fn action(v: maestro_terminal::parser::Action<'_>) -> (&'static str, Value) {
    use maestro_terminal::parser::Action as A;
    match v {
        A::Print(c) => ("print", json!(u32::from(c))),
        A::Execute(c) => ("execute", json!(c)),
        A::CsiDispatch(v) => (
            "csi_dispatch",
            json!({"intermediates":v.intermediates,"params":v.params,"params_sep":{"mask":v.params_sep.0},"final":v.r#final}),
        ),
        A::EscDispatch(v) => (
            "esc_dispatch",
            json!({"intermediates":v.intermediates,"final":v.r#final}),
        ),
        A::DcsHook(v) => (
            "dcs_hook",
            json!({"intermediates":v.intermediates,"params":v.params,"final":v.r#final}),
        ),
        A::OscDispatch(Command::ChangeWindowTitle(v)) => ("osc_dispatch", json!(v)),
        A::OscDispatch(v) => ("osc_dispatch", command(v)),
        A::DcsPut(c) => ("dcs_put", json!(c)),
        A::ApcPut(c) => ("apc_put", json!(c)),
        A::DcsUnhook => ("dcs_unhook", Value::Null),
        A::ApcStart => ("apc_start", Value::Null),
        A::ApcEnd => ("apc_end", Value::Null),
    }
}
