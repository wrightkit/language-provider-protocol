//! Expected-vs-actual response comparison.
//!
//! The fixture DSL marks leaves the spec declares provider-supplied as
//! `{ "$provider": "<kind>" }`. A marked leaf is asserted by its contract
//! shape instead of a literal value, so providers other than the reference
//! mock can satisfy protocol-scope scenarios. Unmarked leaves keep verbatim
//! JSON equality.
//!
//! The kinds mirror spec-declared provider-supplied positions:
//!
//! * `boolean` — a `capabilities` value; the key set in the fixture stays the
//!   normative required-field assertion.
//! * `languageList` — the `languages` array: non-empty, each entry exactly
//!   `{ "id": <non-empty string>, "extensions": [<string>] }`.
//! * `nonEmptyString` — provider prose or identity: `serverInfo` fields,
//!   error `message`, `refusalCode`, provider-defined `reason`.
//! * `protocolVersions` — `supportedProtocolVersions`: a non-empty array of
//!   `MAJOR.MINOR` strings.

use serde_json::Value;

/// The provider-leaf kinds the fixture DSL accepts.
const KINDS: &[&str] = &[
    "boolean",
    "languageList",
    "nonEmptyString",
    "protocolVersions",
];

/// `Some(kind)` when `expected` is a well-formed provider leaf marker.
fn marker_kind(expected: &Value) -> Option<&str> {
    expected
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("$provider"))
        .and_then(Value::as_str)
        .filter(|kind| KINDS.contains(kind))
}

/// Validates every `$provider` marker inside a fixture value. Markers are
/// legal only in expected responses, so requests are checked with
/// `allow_markers: false`.
pub(crate) fn validate_leaves(value: &Value, allow_markers: bool) -> Result<(), String> {
    let is_marker = value
        .as_object()
        .is_some_and(|object| object.contains_key("$provider"));
    if is_marker {
        if !allow_markers {
            return Err("'$provider' markers belong to 'expectResponse'".into());
        }
        return match marker_kind(value) {
            Some(_) => Ok(()),
            None => Err(format!(
                "'$provider' marker must be the object's only key and one of: {}",
                KINDS.join(", ")
            )),
        };
    }
    match value {
        Value::Object(object) => object
            .values()
            .try_for_each(|value| validate_leaves(value, allow_markers)),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_leaves(value, allow_markers)),
        _ => Ok(()),
    }
}

/// Recursive expected-vs-actual comparison honoring provider-leaf markers.
/// On mismatch returns the failing path and what differed.
pub(crate) fn matches(expected: &Value, actual: &Value) -> Result<(), String> {
    match_at(expected, actual, "$")
}

fn match_at(expected: &Value, actual: &Value, path: &str) -> Result<(), String> {
    if let Some(kind) = marker_kind(expected) {
        return check_kind(kind, actual).map_err(|error| format!("{path}: {error}"));
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let Some(actual_value) = actual.get(key) else {
                    return Err(format!("{path}: missing key '{key}'"));
                };
                match_at(expected_value, actual_value, &format!("{path}.{key}"))?;
            }
            for key in actual.keys() {
                if !expected.contains_key(key) {
                    return Err(format!("{path}: unexpected key '{key}'"));
                }
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual)) => {
            if expected.len() != actual.len() {
                return Err(format!(
                    "{path}: expected {} elements, got {}",
                    expected.len(),
                    actual.len()
                ));
            }
            for (i, (expected_value, actual_value)) in
                expected.iter().zip(actual.iter()).enumerate()
            {
                match_at(expected_value, actual_value, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        _ if expected == actual => Ok(()),
        _ => Err(format!("{path}: expected {expected}, got {actual}")),
    }
}

fn check_kind(kind: &str, actual: &Value) -> Result<(), String> {
    let holds = match kind {
        "boolean" => actual.is_boolean(),
        "nonEmptyString" => actual.as_str().is_some_and(|text| !text.is_empty()),
        "languageList" => is_language_list(actual),
        "protocolVersions" => is_version_list(actual),
        _ => unreachable!("validated kinds"),
    };
    if holds {
        Ok(())
    } else {
        Err(format!("expected provider-supplied {kind}, got {actual}"))
    }
}

/// `languages`: a non-empty array of `{ id, extensions }` entries, where `id`
/// is a non-empty string and `extensions` a (possibly empty) string array.
fn is_language_list(actual: &Value) -> bool {
    let Some(entries) = actual.as_array() else {
        return false;
    };
    !entries.is_empty()
        && entries.iter().all(|entry| {
            entry.as_object().is_some_and(|entry| {
                entry.len() == 2
                    && entry
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| !id.is_empty())
                    && entry
                        .get("extensions")
                        .and_then(Value::as_array)
                        .is_some_and(|ext| ext.iter().all(Value::is_string))
            })
        })
}

/// `supportedProtocolVersions`: a non-empty array of `MAJOR.MINOR` strings.
fn is_version_list(actual: &Value) -> bool {
    let Some(versions) = actual.as_array() else {
        return false;
    };
    !versions.is_empty()
        && versions.iter().all(|version| {
            version.as_str().is_some_and(|version| {
                let mut parts = version.split('.');
                matches!(
                    (parts.next(), parts.next(), parts.next()),
                    (Some(major), Some(minor), None)
                        if [major, minor].iter().all(|part| {
                            !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())
                        })
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn verbatim_leaves_compare_exactly() {
        assert!(matches(&json!({"a": [1, 2]}), &json!({"a": [1, 2]})).is_ok());
        let error = matches(&json!({"a": 1}), &json!({"a": 2})).unwrap_err();
        assert!(error.contains("$.a"), "unexpected error: {error}");
    }

    #[test]
    fn provider_leaves_accept_their_shape() {
        for (kind, actual) in [
            ("boolean", json!(false)),
            ("nonEmptyString", json!("any text")),
            (
                "languageList",
                json!([{ "id": "opy", "extensions": ["opy"] }]),
            ),
            ("protocolVersions", json!(["1.0", "1.5"])),
        ] {
            let expected = json!({ "$provider": kind });
            assert!(
                matches(&expected, &actual).is_ok(),
                "{kind} should accept {actual}"
            );
        }
    }

    #[test]
    fn provider_leaves_reject_wrong_shape() {
        for (kind, actual) in [
            ("boolean", json!("true")),
            ("nonEmptyString", json!("")),
            ("nonEmptyString", json!(7)),
            ("languageList", json!([])),
            ("languageList", json!([{ "id": "opy" }])),
            ("languageList", json!([{ "id": "", "extensions": [] }])),
            ("protocolVersions", json!([])),
            ("protocolVersions", json!(["1x"])),
            ("protocolVersions", json!(["1.0", 1])),
        ] {
            let expected = json!({ "$provider": kind });
            assert!(
                matches(&expected, &actual).is_err(),
                "{kind} should reject {actual}"
            );
        }
    }

    #[test]
    fn marker_inside_result_reports_path() {
        let expected = json!({"result": {"serverInfo": {"name": {"$provider": "nonEmptyString"}}}});
        let error =
            matches(&expected, &json!({"result": {"serverInfo": {"name": 42}}})).unwrap_err();
        assert_eq!(
            error,
            "$.result.serverInfo.name: expected provider-supplied nonEmptyString, got 42"
        );
    }

    #[test]
    fn objects_still_require_exact_keys() {
        let expected = json!({"capabilities": {"check": {"$provider": "boolean"}}});
        assert!(
            matches(
                &expected,
                &json!({"capabilities": {"check": true, "lookup": true}})
            )
            .is_err()
        );
        assert!(matches(&expected, &json!({"capabilities": {}})).is_err());
    }

    #[test]
    fn validate_leaves_enforces_marker_form_and_position() {
        assert!(validate_leaves(&json!({"$provider": "boolean"}), true).is_ok());
        assert!(validate_leaves(&json!({"$provider": "unknown"}), true).is_err());
        assert!(validate_leaves(&json!({"$provider": "boolean", "x": 1}), true).is_err());
        assert!(validate_leaves(&json!({"$provider": "boolean"}), false).is_err());
        // Plain objects without the key are never markers.
        assert!(validate_leaves(&json!({"provider": "boolean"}), false).is_ok());
    }
}
