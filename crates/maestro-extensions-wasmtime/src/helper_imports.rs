//! Synchronous helper calls delegate to the native domain without retaining guest operands.
use wasmtime::{StoreContextMut, component::LinkerInstance};

/// Installs ordinary helper calls on the canonical host interface.
pub(crate) fn add_to_linker(
    host: &mut LinkerInstance<'_, crate::lifecycle::State>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "native-format-size",
        |_store: StoreContextMut<'_, crate::lifecycle::State>, (bytes,): (f64,)| {
            Ok((Ok::<_, String>(maestro_extensions::helpers::format_size(
                bytes,
            )),))
        },
    )?;
    host.func_wrap(
        "native-yaml",
        |_store: StoreContextMut<'_, crate::lifecycle::State>, (content,): (String,)| {
            let values = maestro_extensions::helpers::parse_yaml(&content)
                .map_err(|error| error.to_string())
                .and_then(tokens);
            Ok((values,))
        },
    )?;
    host.func_wrap(
        "native-frontmatter",
        |_store: StoreContextMut<'_, crate::lifecycle::State>, (content,): (String,)| {
            Ok((metadata(&content),))
        },
    )?;
    host.func_wrap(
        "native-helper",
        |store: StoreContextMut<'_, crate::lifecycle::State>,
         (operation, data): (String, String)| {
            let value = if operation == "version" {
                serde_json::to_string(&store.data().installation.version)
                    .map_err(|error| error.to_string())
            } else if operation == "config.get_agent_dir" {
                serde_json::to_string(&store.data().installation.agent_dir)
                    .map_err(|error| error.to_string())
            } else {
                maestro_extensions::helpers::call(&operation, &data)
            };
            Ok((value,))
        },
    )
}

/// One native metadata value, lowered directly through the typed interface.
#[derive(wasmtime::component::ComponentType, wasmtime::component::Lower)]
#[component(variant)]
enum FrontmatterToken {
    /// Null scalar.
    #[component(name = "null")]
    Null,
    /// Boolean scalar.
    #[component(name = "boolean")]
    Boolean(bool),
    /// IEEE floating-point scalar, including nonfinite values and signed zero.
    #[component(name = "number")]
    Number(f64),
    /// Text scalar.
    #[component(name = "text")]
    Text(String),
    /// A sequence followed by this many child values.
    #[component(name = "sequence")]
    Sequence(u32),
    /// A mapping followed by one value per key, in authored order.
    #[component(name = "mapping")]
    Mapping(Vec<String>),
}
/// Metadata tree framing and its unmodified native body.
#[derive(wasmtime::component::ComponentType, wasmtime::component::Lower)]
#[component(record)]
struct FrontmatterDocument {
    /// Preorder typed values.
    values: Vec<FrontmatterToken>,
    /// Extracted body.
    body: String,
}
/// Parses through the original native owner, retaining its cause and coordinates.
fn metadata(content: &str) -> Result<FrontmatterDocument, String> {
    let parsed = maestro_extensions::helpers::parse_frontmatter(content)
        .map_err(|error| error.to_string())?;
    Ok(FrontmatterDocument {
        values: tokens(parsed.frontmatter)?,
        body: parsed.body,
    })
}
/// Encodes one native value using the shared typed metadata framing.
fn tokens(
    value: maestro_extensions::helpers::FrontmatterValue,
) -> Result<Vec<FrontmatterToken>, String> {
    let mut values = Vec::new();
    flatten(value, &mut values)?;
    Ok(values)
}
/// Frames a native tree without converting its numbers to JSON.
fn flatten(
    value: maestro_extensions::helpers::FrontmatterValue,
    values: &mut Vec<FrontmatterToken>,
) -> Result<(), String> {
    use maestro_extensions::helpers::FrontmatterValue as V;
    let token = match value {
        V::Null => FrontmatterToken::Null,
        V::Bool(value) => FrontmatterToken::Boolean(value),
        V::Number(value) => FrontmatterToken::Number(value),
        V::String(value) => FrontmatterToken::Text(value),
        V::Sequence(items) => {
            values.push(FrontmatterToken::Sequence(
                u32::try_from(items.len()).map_err(|error| error.to_string())?,
            ));
            for item in items {
                flatten(item, values)?;
            }
            return Ok(());
        }
        V::Mapping(entries) => {
            let (keys, items): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
            values.push(FrontmatterToken::Mapping(keys));
            for item in items {
                flatten(item, values)?;
            }
            return Ok(());
        }
    };
    values.push(token);
    Ok(())
}
