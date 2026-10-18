//! Raw viewer operands and reached failure boundaries.
#[path = "../../../tests/export_support/corpus.rs"]
mod fixtures;

#[test]
fn export_display_string_keeps_missing_null_and_invalid_distinct() {
    for row in fixtures::rows(
        "export_display_string_keeps_missing_null_and_invalid_distinct",
        "str",
    ) {
        assert_eq!(row.branch, "success");
        let result = super::values::str(row.input.get("value"));
        assert_eq!(result, row.expected["value"].as_str(), "{}", row.id);
    }
}

#[test]
fn export_path_language_reuses_only_viewer_suffixes() {
    for row in fixtures::rows(
        "export_path_language_reuses_only_viewer_suffixes",
        "language",
    ) {
        assert_eq!(row.branch, "success");
        assert_eq!(
            super::values::get_language_from_path(row.input["path"].as_str().unwrap()),
            row.expected["value"].as_str(),
            "{}",
            row.id
        );
    }
}

#[test]
fn export_timestamp_uses_owning_native_time_options() {
    for row in fixtures::rows(
        "export_timestamp_uses_owning_native_time_options",
        "timestamp",
    ) {
        assert_eq!(row.branch, "success");
        let calls = std::cell::Cell::new(0);
        let result = super::values::format_timestamp(row.input.get("value"), &|value| {
            calls.set(calls.get() + 1);
            assert_eq!(Some(value), row.input.get("value"));
            Ok(row.expected["value"].as_str().unwrap().to_owned())
        })
        .unwrap();
        assert_eq!(
            result,
            row.expected["value"].as_str().unwrap(),
            "{}",
            row.id
        );
        assert_eq!(
            calls.get(),
            usize::from(!row.expected["value"].as_str().unwrap().is_empty()),
            "{}",
            row.id
        );
    }
    let value = serde_json::json!(1);
    assert_eq!(
        super::values::format_timestamp(Some(&value), &|_| Err(
            std::io::ErrorKind::PermissionDenied.into()
        ))
        .unwrap_err()
        .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn export_tool_results_keep_first_match_and_reached_errors() {
    for row in fixtures::rows(
        "export_tool_results_keep_first_match_and_reached_errors",
        "result",
    ) {
        let mut document = super::load_document("eyJlbnRyaWVzIjpbXX0=", "", None).unwrap();
        document
            .data
            .insert("entries".to_owned(), row.input["entries"].clone());
        let result = super::values::find_tool_result(&document, row.input["id"].as_str().unwrap());
        if row.branch == "error" {
            assert_eq!(row.expected["kind"], "InvalidData");
            assert_eq!(
                result.unwrap_err().kind(),
                std::io::ErrorKind::InvalidData,
                "{}",
                row.id
            );
        } else {
            assert_eq!(row.branch, "success");
            assert_eq!(
                result.unwrap(),
                row.expected.get("value").filter(|v| !v.is_null()),
                "{}",
                row.id
            );
        }
    }
}

/// Native time formatting observes raw values and undefined locale with exact options.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn export_timestamps_use_browser_locale_and_raw_values() {
    use wasm_bindgen::JsValue;
    for row in fixtures::rows(
        "export_timestamp_uses_owning_native_time_options",
        "timestamp",
    ) {
        let input = row.input.get("value");
        let observed =
            super::values::format_timestamp(input, &super::values::localize_timestamp).unwrap();
        js_sys::Reflect::set(
            &js_sys::global(),
            &"timestampJson".into(),
            &row.input.to_string().into(),
        )
        .unwrap();
        let expected = js_sys::eval("(()=>{const v=JSON.parse(timestampJson).value;return v?new Date(v).toLocaleTimeString(undefined,{hour:'2-digit',minute:'2-digit',second:'2-digit'}):''})()").unwrap().as_string().unwrap();
        assert_eq!(observed, expected, "{}", row.id);
        assert_eq!(
            observed.is_empty(),
            row.expected["value"] == "",
            "{}",
            row.id
        );
        assert_eq!(row.branch, "success");
    }
    let restore = js_sys::eval("(()=>{const old=Date.prototype.toLocaleTimeString;Date.prototype.toLocaleTimeString=function(locale,options){if(locale!==undefined||JSON.stringify(options)!==JSON.stringify({hour:'2-digit',minute:'2-digit',second:'2-digit'}))throw Error('wrong native time operands');return old.call(this,locale,options)};return ()=>Date.prototype.toLocaleTimeString=old})()").unwrap();
    let result = super::values::localize_timestamp(&serde_json::json!(1));
    use wasm_bindgen::JsCast as _;
    restore
        .unchecked_into::<js_sys::Function>()
        .call0(&JsValue::UNDEFINED)
        .unwrap();
    assert!(!result.unwrap().is_empty());
}
