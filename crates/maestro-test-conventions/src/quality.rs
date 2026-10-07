use std::path::Path;

use serde_json::Value;

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
            check_allowances(&path, &contents)?;
            if path.file_name().is_some_and(|name| name == "tests.rs")
                || path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .components()
                    .any(|part| part.as_os_str() == "tests")
            {
                continue;
            }
            let lines = production_lines(&contents);
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

fn production_lines(contents: &str) -> usize {
    let tokens = source::tokens(contents);
    let prefix = TEST_MODULE;
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate() {
        if depth == 0
            && tokens[index..].iter().zip(prefix).all(|(t, p)| t.text == p)
            && tokens.len() >= index + prefix.len()
            && trailing_module(&tokens[index + prefix.len()..])
        {
            return source::line(contents, token.start) - 1;
        }
        match token.text {
            "{" => depth += 1,
            "}" => depth -= 1,
            _ => {}
        }
    }
    contents.lines().count()
}

const TEST_MODULE: [&str; 8] = ["#", "[", "cfg", "(", "test", ")", "]", "mod"];

fn trailing_module(tokens: &[source::Token<'_>]) -> bool {
    let Some(end) = module_end(tokens) else {
        return false;
    };
    let remaining = &tokens[end..];
    remaining.is_empty()
        || (remaining.len() >= TEST_MODULE.len()
            && remaining
                .iter()
                .zip(TEST_MODULE)
                .all(|(token, text)| token.text == text)
            && trailing_module(&remaining[TEST_MODULE.len()..]))
}

fn module_end(tokens: &[source::Token<'_>]) -> Option<usize> {
    if tokens.get(1).is_some_and(|token| token.text == ";") {
        return Some(2);
    }
    if tokens.get(1).is_none_or(|token| token.text != "{") {
        return None;
    }
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(1) {
        match token.text {
            "{" => depth += 1,
            "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(index + 1);
        }
    }
    None
}

fn check_inheritance(name: &str, manifest: &Path) -> Result<(), String> {
    let contents = std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    let mut in_lints = false;
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_lints = line == "[lints]";
        } else if in_lints
            && line
                .split_once('=')
                .is_some_and(|(key, value)| key.trim() == "workspace" && value.trim() == "true")
        {
            return Ok(());
        }
    }
    Err(format!("{name}: must inherit workspace lints"))
}

fn check_allowances(path: &Path, contents: &str) -> Result<(), String> {
    let tokens = source::tokens(contents);
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "#" {
            continue;
        }
        let mut start = index + 1;
        if tokens.get(start).is_some_and(|token| token.text == "!") {
            start += 1;
        }
        if tokens.get(start).is_none_or(|token| token.text != "[") {
            continue;
        }
        let attribute = &tokens[start + 1..];
        if forbidden_allowance(attribute) {
            return Err(format!(
                "{}:{}: quality lint allowance is forbidden",
                path.display(),
                source::line(contents, token.start)
            ));
        }
    }
    Ok(())
}

fn forbidden_allowance(attribute: &[source::Token<'_>]) -> bool {
    let mut allowing = false;
    for token in attribute.iter().take_while(|token| token.text != "]") {
        if matches!(token.text, "allow" | "expect") {
            allowing = true;
        } else if token.text == ")" {
            allowing = false;
        } else if allowing
            && (PEDANTIC.contains(&token.text)
                || matches!(
                    token.text,
                    "too_many_arguments"
                        | "fn_params_excessive_bools"
                        | "too_many_lines"
                        | "cognitive_complexity"
                        | "excessive_nesting"
                        | "pedantic"
                        | "unwrap_used"
                        | "expect_used"
                        | "panic"
                        | "unsafe_code"
                        | "warnings"
                        | "all"
                ))
        {
            return true;
        }
    }
    false
}

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
