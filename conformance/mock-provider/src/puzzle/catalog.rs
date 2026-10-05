//! The `x-demo-lang` name vocabulary served by `lpp/lookup`.
//!
//! The catalog is fixed for a provider version: the section keywords
//! (`puzzle`, `ops`, `solution`), the four arithmetic operators and their
//! enum domain, and the `target`/`start` settings. List order is the
//! language's canonical order and is also the tie-break order for ranked
//! results.

use serde_json::{Value, json};

/// A lookup scope: the full catalog or the children of one `within` selector.
pub(crate) enum LookupScope {
    All,
    Callable(String),
    Enum(String),
    Settings(String),
}

/// Ranked `entries` for an `lpp/lookup` request. Returns `None` when `scope`
/// names a `within` selector the language does not know; the caller turns
/// that into a `lookup.unknownWithin` refusal.
pub(crate) fn lookup(
    query: &str,
    kind: Option<&str>,
    scope: &LookupScope,
    limit: usize,
) -> Option<Vec<Value>> {
    let entries = match scope {
        LookupScope::All => catalog(),
        LookupScope::Callable(identity) => callable_params(identity)?,
        LookupScope::Enum(identity) => enum_members(identity)?,
        LookupScope::Settings(prefix) => settings_children(prefix)?,
    };
    Some(rank(
        entries,
        query.trim().to_lowercase().as_str(),
        kind,
        limit,
    ))
}

/// Keep `kind` matches, drop non-matches for `query`, order by match tier
/// then catalog order (the sort is stable), and cap at `limit` entries.
fn rank(entries: Vec<Value>, query: &str, kind: Option<&str>, limit: usize) -> Vec<Value> {
    let mut scored: Vec<(u8, Value)> = entries
        .into_iter()
        .filter(|e| kind.is_none_or(|k| e["kind"] == k))
        .filter_map(|e| score(&e, query).map(|s| (s, e)))
        .collect();
    scored.sort_by_key(|(s, _)| *s);
    scored.truncate(limit);
    scored.into_iter().map(|(_, e)| e).collect()
}

/// Match tier: 0 exact, 1 prefix, 2 substring; `None` = no match. An empty
/// query matches everything at tier 0, so an unconstrained result keeps the
/// scope's canonical order.
fn score(entry: &Value, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    [entry["spelling"].as_str(), entry["displayName"].as_str()]
        .into_iter()
        .flatten()
        .map(str::to_lowercase)
        .filter_map(|text| {
            if text == query {
                Some(0)
            } else if text.starts_with(query) {
                Some(1)
            } else if text.contains(query) {
                Some(2)
            } else {
                None
            }
        })
        .min()
}

fn catalog() -> Vec<Value> {
    vec![
        keyword("puzzle"),
        keyword("ops"),
        keyword("solution"),
        operator("add", "+"),
        operator("subtract", "-"),
        operator("multiply", "*"),
        operator("divide", "/"),
        operator_domain(),
        setting("target"),
        setting("start"),
    ]
}

fn keyword(name: &str) -> Value {
    json!({
        "identity": format!("x-demo:keyword/{name}"),
        "kind": "keyword",
        "spelling": name,
        "displayName": name,
    })
}

/// An arithmetic operator: `x <symbol> <arg>` inside an op declaration, so it
/// is callable on an `integer` receiver with one required `arg` parameter.
fn operator(name: &str, symbol: &str) -> Value {
    json!({
        "identity": format!("x-demo:operator/{name}"),
        "kind": "operator",
        "spelling": symbol,
        "displayName": name,
        "callable": {
            "receiver": "integer",
            "parameters": [
                { "name": "arg", "type": "integer", "required": true }
            ]
        }
    })
}

/// The enum domain of arithmetic operators op bodies choose from.
fn operator_domain() -> Value {
    json!({
        "identity": "x-demo:enum/operator",
        "kind": "enum",
        "spelling": "operator",
        "displayName": "arithmetic operator",
        "enum": {
            "domain": "x-demo:enum/operator",
            "members": ["+", "-", "*", "/"]
        }
    })
}

/// A required `name = <integer>` assignment in the puzzle body.
fn setting(name: &str) -> Value {
    json!({
        "identity": format!("x-demo:setting/{name}"),
        "kind": "setting",
        "spelling": name,
        "displayName": name,
        "setting": { "type": "integer" }
    })
}

/// One entry per parameter of the callable at `identity`, in call order.
/// `None` when `identity` names no catalog callable.
fn callable_params(identity: &str) -> Option<Vec<Value>> {
    let parameters =
        catalog().into_iter().find(|e| e["identity"] == identity)?["callable"]["parameters"]
            .as_array()?
            .clone();
    Some(
        parameters
            .iter()
            .map(|p| {
                let mut facts = p.clone();
                facts
                    .as_object_mut()
                    .expect("parameter object")
                    .remove("name");
                json!({
                    "identity": format!("{identity}/{}", p["name"].as_str().unwrap_or("parameter")),
                    "kind": "parameter",
                    "spelling": p["name"],
                    "displayName": p["name"],
                    "parameter": facts,
                })
            })
            .collect(),
    )
}

/// The member spellings of the enum domain at `identity`, in domain order.
/// `None` when `identity` names no entry that defines an enum domain.
fn enum_members(identity: &str) -> Option<Vec<Value>> {
    let members = catalog()
        .into_iter()
        .find(|e| e["enum"]["domain"] == identity)?["enum"]["members"]
        .as_array()?
        .clone();
    Some(
        members
            .iter()
            .map(|m| {
                let spelling = m.as_str().unwrap_or_default();
                json!({
                    "identity": format!("{identity}/{spelling}"),
                    "kind": "enumMember",
                    "spelling": spelling,
                    "displayName": display_name(spelling),
                })
            })
            .collect(),
    )
}

/// Entries directly below a settings path prefix. `x-demo-lang` settings are
/// a flat namespace, so only the empty prefix names a scope.
fn settings_children(prefix: &str) -> Option<Vec<Value>> {
    if !prefix.is_empty() {
        return None;
    }
    Some(
        catalog()
            .into_iter()
            .filter(|e| e.get("setting").is_some())
            .collect(),
    )
}

/// The display name of the catalog entry with `spelling`, else the spelling.
fn display_name(spelling: &str) -> String {
    catalog()
        .into_iter()
        .find(|e| e["spelling"] == spelling)
        .and_then(|e| e["displayName"].as_str().map(str::to_string))
        .unwrap_or_else(|| spelling.to_string())
}
