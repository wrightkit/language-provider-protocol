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

/// The `capabilities` object inside an `lpp/initialize` result.
const CAPABILITIES_PATH: &str = "$.result.capabilities";

/// Capability ids in introduction order. Per §7.3 the set is closed for each
/// LPP minor version: ids are added only through a new protocol version.
const CAPABILITY_IDS: &[&str] = &[
    "check",
    "compile",
    "reconstruct",
    "symbols",
    "definition",
    "references",
    "rename",
    "editValidation",
    "projectLoading",
    "sourceIdentity",
    "lookup",
];

/// The capability ids a negotiated protocol version declares.
fn declared_capability_ids(root: &Value) -> Option<&'static [&'static str]> {
    let count = match root.pointer("/result/protocolVersion")?.as_str()? {
        "1.0" => 8,
        "1.1" | "1.2" => 9,
        "1.3" | "1.4" => 10,
        "1.5" => 11,
        _ => return None,
    };
    Some(&CAPABILITY_IDS[..count])
}

/// The `data.lpp.kind` of an expected error response.
fn expected_error_kind(root: &Value) -> Option<&str> {
    root.pointer("/error/data/lpp/kind")?.as_str()
}

/// Whether `{ "$provider": kind }` may appear at `path` inside an expected
/// response, per the §20.1 whitelist of spec-declared provider-supplied
/// positions. `root` is the whole `expectResponse`, so positions whose
/// legality depends on sibling fields (`data.lpp.kind`, the negotiated
/// `protocolVersion`) can be checked.
fn marker_position_legal(path: &str, kind: &str, root: &Value) -> bool {
    match path {
        "$.result.serverInfo.name" | "$.result.serverInfo.version" => kind == "nonEmptyString",
        "$.result.languages" => kind == "languageList",
        "$.error.message" => kind == "nonEmptyString",
        "$.error.data.lpp.details.supportedProtocolVersions" => {
            kind == "protocolVersions"
                && expected_error_kind(root) == Some("protocolVersionMismatch")
        }
        "$.error.data.lpp.details.refusalCode" => {
            kind == "nonEmptyString" && expected_error_kind(root) == Some("refusal")
        }
        // `reason` is provider-defined only for the kinds whose details the
        // spec does not enumerate; `invalidRequest.reason` is a closed set.
        "$.error.data.lpp.details.reason" => {
            kind == "nonEmptyString"
                && matches!(
                    expected_error_kind(root),
                    Some(
                        "invalidDocument"
                            | "invalidEntry"
                            | "projectLoadFailed"
                            | "invalidArtifact"
                    )
                )
        }
        _ => match path
            .strip_prefix(CAPABILITIES_PATH)
            .and_then(|id| id.strip_prefix('.'))
        {
            Some(id) => {
                kind == "boolean"
                    && declared_capability_ids(root).is_some_and(|ids| ids.contains(&id))
            }
            None => false,
        },
    }
}

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
/// `allow_markers: false`. A marker's legality also depends on where it sits:
/// `value` doubles as the response root so marker paths can be checked
/// against the §20.1 whitelist.
pub(crate) fn validate_leaves(value: &Value, allow_markers: bool) -> Result<(), String> {
    validate_at(value, value, "$", allow_markers)
}

fn validate_at(root: &Value, value: &Value, path: &str, allow_markers: bool) -> Result<(), String> {
    let is_marker = value
        .as_object()
        .is_some_and(|object| object.contains_key("$provider"));
    if is_marker {
        if !allow_markers {
            return Err("'$provider' markers belong to 'expectResponse'".into());
        }
        let Some(kind) = marker_kind(value) else {
            return Err(format!(
                "'$provider' marker must be the object's only key and one of: {}",
                KINDS.join(", ")
            ));
        };
        return marker_position_legal(path, kind, root)
            .then_some(())
            .ok_or_else(|| {
                format!("'{kind}' marker is not a provider-supplied position at '{path}'")
            });
    }
    if path == CAPABILITIES_PATH {
        validate_capability_object(root, value)?;
    }
    match value {
        Value::Object(object) => object.iter().try_for_each(|(key, value)| {
            validate_at(root, value, &format!("{path}.{key}"), allow_markers)
        }),
        Value::Array(values) => values.iter().enumerate().try_for_each(|(i, value)| {
            validate_at(root, value, &format!("{path}[{i}]"), allow_markers)
        }),
        _ => Ok(()),
    }
}

/// A `capabilities` object that carries markers must mark the full declared
/// capability set for the negotiated version: every value a `boolean` marker,
/// every key a declared id, and no declared id missing. Verbatim capability
/// maps (adapter-specific `providerArgs` scenarios) are unaffected.
fn validate_capability_object(root: &Value, value: &Value) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let marked = object.values().any(|value| {
        value
            .as_object()
            .is_some_and(|o| o.contains_key("$provider"))
    });
    if !marked {
        return Ok(());
    }
    if !object
        .values()
        .all(|value| marker_kind(value) == Some("boolean"))
    {
        return Err(format!(
            "'{CAPABILITIES_PATH}': provider markers cannot mix with verbatim capability values"
        ));
    }
    let declared = declared_capability_ids(root).unwrap_or(&[]);
    if object.len() != declared.len() || !object.keys().all(|key| declared.contains(&key.as_str()))
    {
        return Err(format!(
            "'{CAPABILITIES_PATH}': marked capabilities must cover exactly the declared ids for the negotiated version"
        ));
    }
    Ok(())
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
                    // §7.3: an absent capability id means not advertised —
                    // equivalent to `false`. Everywhere else keys are required.
                    if path == CAPABILITIES_PATH {
                        continue;
                    }
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
/// is a non-empty string and `extensions` a (possibly empty) array of file
/// extensions written without a leading dot and in lowercase (§7.2).
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
                        .is_some_and(|ext| {
                            ext.iter().all(|ext| ext.as_str().is_some_and(is_extension))
                        })
            })
        })
}

fn is_extension(ext: &str) -> bool {
    !ext.is_empty() && !ext.starts_with('.') && !ext.chars().any(char::is_uppercase)
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
            // §7.2: extensions are lowercase, without a leading dot.
            (
                "languageList",
                json!([{ "id": "opy", "extensions": [".OPY"] }]),
            ),
            (
                "languageList",
                json!([{ "id": "opy", "extensions": ["OPY"] }]),
            ),
            ("languageList", json!([{ "id": "opy", "extensions": [""] }])),
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
    }

    #[test]
    fn capabilities_may_advertise_a_subset() {
        let expected = json!({"result": {"capabilities": {"check": {"$provider": "boolean"}, "lookup": {"$provider": "boolean"}}}});
        // Per §7.3 absent means not advertised; advertised values are checked.
        assert!(matches(&expected, &json!({"result": {"capabilities": {}}})).is_ok());
        assert!(
            matches(
                &expected,
                &json!({"result": {"capabilities": {"check": false}}})
            )
            .is_ok()
        );
        assert!(
            matches(
                &expected,
                &json!({"result": {"capabilities": {"check": true, "custom": true}}})
            )
            .is_err(),
            "undeclared capability ids violate the closed set"
        );
        assert!(
            matches(
                &expected,
                &json!({"result": {"capabilities": {"check": "yes"}}})
            )
            .is_err()
        );
    }

    fn init_result() -> Value {
        json!({
            "result": {
                "protocolVersion": "1.5",
                "serverInfo": {
                    "name": {"$provider": "nonEmptyString"},
                    "version": {"$provider": "nonEmptyString"}
                },
                "languages": {"$provider": "languageList"},
                "capabilities": {
                    "check": {"$provider": "boolean"},
                    "compile": {"$provider": "boolean"},
                    "reconstruct": {"$provider": "boolean"},
                    "symbols": {"$provider": "boolean"},
                    "definition": {"$provider": "boolean"},
                    "references": {"$provider": "boolean"},
                    "rename": {"$provider": "boolean"},
                    "editValidation": {"$provider": "boolean"},
                    "projectLoading": {"$provider": "boolean"},
                    "sourceIdentity": {"$provider": "boolean"},
                    "lookup": {"$provider": "boolean"}
                }
            }
        })
    }

    #[test]
    fn validate_leaves_enforces_marker_form_and_position() {
        assert!(validate_leaves(&init_result(), true).is_ok());
        // Marker form.
        assert!(validate_leaves(&json!({"result": {"protocolVersion": "1.5", "capabilities": {"check": {"$provider": "unknown"}}}}), true).is_err());
        assert!(validate_leaves(&json!({"$provider": "boolean", "x": 1}), true).is_err());
        assert!(validate_leaves(&json!({"$provider": "boolean"}), false).is_err());
        // Plain objects without the key are never markers.
        assert!(validate_leaves(&json!({"provider": "boolean"}), false).is_ok());
    }

    #[test]
    fn markers_reject_contract_owned_positions() {
        // Marker on the root or a contract-owned leaf.
        for expected in [
            json!({"$provider": "nonEmptyString"}),
            json!({"result": {"protocolVersion": {"$provider": "nonEmptyString"}}}),
            json!({"error": {"code": {"$provider": "nonEmptyString"}}}),
            json!({"error": {"data": {"lpp": {"kind": {"$provider": "nonEmptyString"}}}}}),
            json!({"result": {"serverInfo": {"$provider": "nonEmptyString"}}}),
        ] {
            assert!(
                validate_leaves(&expected, true).is_err(),
                "marker should be rejected: {expected}"
            );
        }
    }

    #[test]
    fn marker_kind_must_match_position() {
        let mut expected = init_result();
        expected["result"]["languages"] = json!({"$provider": "nonEmptyString"});
        assert!(validate_leaves(&expected, true).is_err());
        expected["result"]["serverInfo"]["name"] = json!({"$provider": "boolean"});
        assert!(validate_leaves(&expected, true).is_err());
    }

    #[test]
    fn details_markers_follow_the_error_kind() {
        // `reason` is provider-defined under `invalidDocument` ...
        let expected = json!({"error": {"data": {"lpp": {
            "kind": "invalidDocument",
            "details": {"reason": {"$provider": "nonEmptyString"}}
        }}}});
        assert!(validate_leaves(&expected, true).is_ok());
        // ... but a closed set under `invalidRequest`.
        let expected = json!({"error": {"data": {"lpp": {
            "kind": "invalidRequest",
            "details": {"reason": {"$provider": "nonEmptyString"}}
        }}}});
        assert!(validate_leaves(&expected, true).is_err());
        // `refusalCode` only under `refusal`; `supportedProtocolVersions` only
        // under `protocolVersionMismatch`.
        let expected = json!({"error": {"data": {"lpp": {
            "kind": "invalidRequest",
            "details": {"refusalCode": {"$provider": "nonEmptyString"}}
        }}}});
        assert!(validate_leaves(&expected, true).is_err());
        let expected = json!({"error": {"data": {"lpp": {
            "kind": "refusal",
            "details": {"supportedProtocolVersions": {"$provider": "protocolVersions"}}
        }}}});
        assert!(validate_leaves(&expected, true).is_err());
    }

    #[test]
    fn marked_capabilities_cover_the_declared_set() {
        let mut expected = init_result();
        // A declared id left unmarked weakens the closed-set check.
        expected["result"]["capabilities"]
            .as_object_mut()
            .unwrap()
            .remove("lookup");
        assert!(validate_leaves(&expected, true).is_err());
        // An undeclared id is not a provider-supplied position.
        let mut expected = init_result();
        expected["result"]["capabilities"]
            .as_object_mut()
            .unwrap()
            .insert("custom".into(), json!({"$provider": "boolean"}));
        assert!(validate_leaves(&expected, true).is_err());
        // Markers cannot mix with verbatim capability values.
        let mut expected = init_result();
        expected["result"]["capabilities"]["check"] = json!(true);
        assert!(validate_leaves(&expected, true).is_err());
        // The declared set follows the negotiated version.
        let mut expected = init_result();
        expected["result"]["protocolVersion"] = json!("1.0");
        for key in ["projectLoading", "sourceIdentity", "lookup"] {
            expected["result"]["capabilities"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        }
        assert!(validate_leaves(&expected, true).is_ok());
    }
}
