use std::path::Path;

use serde_json::Value;
use syn::{Meta, Token, punctuated::Punctuated, spanned::Spanned, visit::Visit};

use crate::{array, source, string};

pub(crate) fn check(metadata: &Value) -> Result<(), String> {
    let members = array(metadata, "workspace_members")?;
    for package in array(metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let name = string(package, "name")?;
        let manifest = Path::new(string(package, "manifest_path")?);
        let root = manifest.parent().ok_or("manifest has no parent")?;
        check_inheritance(name, manifest)?;
        for path in source::files(root)? {
            let contents = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let syntax = syn::parse_file(&contents)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            check_allowances(&path, &syntax)?;
            if path.file_name().is_some_and(|name| name == "tests.rs")
                || path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .components()
                    .any(|part| part.as_os_str() == "tests")
            {
                continue;
            }
            let lines = production_lines(&contents, &syntax);
            if lines > 500 {
                return Err(format!(
                    "{}: {lines} production lines exceeds 500",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn production_lines(contents: &str, syntax: &syn::File) -> usize {
    let mut tests = TestLines::default();
    tests.visit_file(syntax);
    let end = syntax
        .items
        .last()
        .filter(|item| is_test(item_attributes(item)))
        .map_or(contents.lines().count(), |item| item.span().end().line);
    (1..=end)
        .filter(|line| !tests.spans.iter().any(|span| span.contains(line)))
        .count()
}

#[derive(Default)]
struct TestLines {
    spans: Vec<std::ops::RangeInclusive<usize>>,
}

impl<'ast> Visit<'ast> for TestLines {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if is_test(item_attributes(item)) {
            self.spans
                .push(item.span().start().line..=item.span().end().line);
        } else {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        let attributes = match item {
            syn::ImplItem::Const(item) => &item.attrs,
            syn::ImplItem::Fn(item) => &item.attrs,
            syn::ImplItem::Type(item) => &item.attrs,
            syn::ImplItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if is_test(attributes) {
            self.spans
                .push(item.span().start().line..=item.span().end().line);
        } else {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        let attributes = match item {
            syn::TraitItem::Const(item) => &item.attrs,
            syn::TraitItem::Fn(item) => &item.attrs,
            syn::TraitItem::Type(item) => &item.attrs,
            syn::TraitItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if is_test(attributes) {
            self.spans
                .push(item.span().start().line..=item.span().end().line);
        } else {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        let attributes = match item {
            syn::ForeignItem::Fn(item) => &item.attrs,
            syn::ForeignItem::Static(item) => &item.attrs,
            syn::ForeignItem::Type(item) => &item.attrs,
            syn::ForeignItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if is_test(attributes) {
            self.spans
                .push(item.span().start().line..=item.span().end().line);
        } else {
            syn::visit::visit_foreign_item(self, item);
        }
    }
}

fn is_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn check_inheritance(name: &str, manifest: &Path) -> Result<(), String> {
    let contents = std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    let document: toml::Table = contents
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    if document
        .get("lints")
        .and_then(toml::Value::as_table)
        .and_then(|lints| lints.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
    {
        return Ok(());
    }
    Err(format!("{name}: must inherit workspace lints"))
}

fn check_allowances(path: &Path, syntax: &syn::File) -> Result<(), String> {
    let mut visitor = Allowances { error: None };
    visitor.visit_file(syntax);
    visitor
        .error
        .map_or(Ok(()), |error| Err(format!("{}:{error}", path.display())))
}

struct Allowances {
    error: Option<String>,
}

impl<'ast> Visit<'ast> for Allowances {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        match forbidden_allowance(&attribute.meta) {
            Ok(true) => {
                self.error = Some(format!(
                    "{}: quality lint allowance is forbidden",
                    attribute.span().start().line
                ));
            }
            Err(error) => self.error = Some(error.to_string()),
            Ok(false) => {}
        }
        syn::visit::visit_attribute(self, attribute);
    }
}

fn forbidden_allowance(meta: &Meta) -> syn::Result<bool> {
    let Meta::List(list) = meta else {
        return Ok(false);
    };
    if list.path.is_ident("cfg_attr") {
        let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for attribute in nested.iter().skip(1) {
            if forbidden_allowance(attribute)? {
                return Ok(true);
            }
        }
    } else if list.path.is_ident("allow") || list.path.is_ident("expect") {
        let lints = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        return Ok(lints.iter().any(|lint| {
            lint.path().segments.last().is_some_and(|segment| {
                let name = segment.ident.to_string();
                PEDANTIC.contains(&name.as_str())
                    || GROUPS.contains(&name.as_str())
                    || PROTECTED.contains(&name.as_str())
            })
        }));
    }
    Ok(false)
}

const GROUPS: &[&str] = &[
    "all",
    "pedantic",
    "restriction",
    "nursery",
    "cargo",
    "complexity",
    "correctness",
    "perf",
    "style",
    "suspicious",
    "warnings",
];

const PROTECTED: &[&str] = &[
    "too_many_arguments",
    "fn_params_excessive_bools",
    "too_many_lines",
    "cognitive_complexity",
    "excessive_nesting",
    "unwrap_used",
    "expect_used",
    "panic",
    "unsafe_code",
];

// Members of the pedantic group in the pinned Rust toolchain.
const PEDANTIC: &[&str] = &[
    "assigning_clones",
    "bool_to_int_with_if",
    "borrow_as_ptr",
    "case_sensitive_file_extension_comparisons",
    "cast_lossless",
    "cast_possible_truncation",
    "cast_possible_wrap",
    "cast_precision_loss",
    "cast_ptr_alignment",
    "cast_sign_loss",
    "checked_conversions",
    "cloned_instead_of_copied",
    "collapsible_else_if",
    "comparison_chain",
    "copy_iterator",
    "decimal_bitwise_operands",
    "default_trait_access",
    "doc_broken_link",
    "doc_comment_double_space_linebreaks",
    "doc_link_with_quotes",
    "doc_markdown",
    "duration_suboptimal_units",
    "elidable_lifetime_names",
    "enum_glob_use",
    "expl_impl_clone_on_copy",
    "explicit_deref_methods",
    "explicit_into_iter_loop",
    "explicit_iter_loop",
    "filter_map_next",
    "flat_map_option",
    "float_cmp",
    "fn_params_excessive_bools",
    "format_collect",
    "format_push_string",
    "if_not_else",
    "ignore_without_reason",
    "ignored_unit_patterns",
    "implicit_clone",
    "implicit_hasher",
    "inconsistent_struct_constructor",
    "index_refutable_slice",
    "inefficient_to_string",
    "inline_always",
    "into_iter_without_iter",
    "invalid_upcast_comparisons",
    "ip_constant",
    "items_after_statements",
    "iter_filter_is_ok",
    "iter_filter_is_some",
    "iter_not_returning_iterator",
    "iter_without_into_iter",
    "large_digit_groups",
    "large_futures",
    "large_stack_arrays",
    "large_types_passed_by_value",
    "linkedlist",
    "macro_use_imports",
    "manual_assert",
    "manual_assert_eq",
    "manual_ilog",
    "manual_instant_elapsed",
    "manual_is_power_of_two",
    "manual_is_variant_and",
    "manual_let_else",
    "manual_midpoint",
    "manual_string_new",
    "many_single_char_names",
    "map_unwrap_or",
    "match_bool",
    "match_same_arms",
    "match_wild_err_arm",
    "match_wildcard_for_single_variants",
    "maybe_infinite_iter",
    "mismatching_type_param_order",
    "missing_errors_doc",
    "missing_fields_in_debug",
    "missing_panics_doc",
    "must_use_candidate",
    "mut_mut",
    "naive_bytecount",
    "needless_bitwise_bool",
    "needless_continue",
    "needless_for_each",
    "needless_pass_by_value",
    "needless_raw_string_hashes",
    "no_effect_underscore_binding",
    "no_mangle_with_rust_abi",
    "non_std_lazy_statics",
    "nonminimal_bool",
    "option_as_ref_cloned",
    "option_option",
    "overly_complex_bool_expr",
    "ptr_as_ptr",
    "ptr_cast_constness",
    "ptr_offset_by_literal",
    "pub_underscore_fields",
    "range_minus_one",
    "range_plus_one",
    "redundant_closure_for_method_calls",
    "redundant_else",
    "ref_as_ptr",
    "ref_binding_to_reference",
    "ref_option",
    "ref_option_ref",
    "return_self_not_must_use",
    "same_functions_in_if_condition",
    "same_length_and_capacity",
    "self_only_used_in_recursion",
    "semicolon_if_nothing_returned",
    "should_panic_without_expect",
    "similar_names",
    "single_char_pattern",
    "single_match_else",
    "stable_sort_primitive",
    "str_split_at_newline",
    "string_add_assign",
    "struct_excessive_bools",
    "struct_field_names",
    "too_many_lines",
    "transmute_ptr_to_ptr",
    "trivially_copy_pass_by_ref",
    "unchecked_time_subtraction",
    "unicode_not_nfc",
    "uninlined_format_args",
    "unnecessary_box_returns",
    "unnecessary_debug_formatting",
    "unnecessary_join",
    "unnecessary_literal_bound",
    "unnecessary_semicolon",
    "unnecessary_trailing_comma",
    "unnecessary_wraps",
    "unnested_or_patterns",
    "unreadable_literal",
    "unsafe_derive_deserialize",
    "unused_async",
    "unused_async_trait_impl",
    "unused_self",
    "used_underscore_binding",
    "used_underscore_items",
    "verbose_bit_mask",
    "wildcard_imports",
    "with_capacity_zero",
    "zero_sized_map_values",
];
