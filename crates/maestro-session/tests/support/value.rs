//! Complete JSON fixture comparison.
use serde_json::Value;
/// Compare complete JSON results using the shared numeric domain.
pub fn assert_value(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{label}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (a, b) in a.iter().zip(b) {
                assert_value(a, b, label);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (key, value) in b {
                assert_value(
                    a.get(key)
                        .unwrap_or_else(|| panic!("{label}: absent {key}")),
                    value,
                    label,
                );
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}
