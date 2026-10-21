use serde_json::{Value, json};

pub(crate) enum LookupScope {
    All,
    Callable(String),
    Enum(String),
    Settings(String),
}

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

fn setting(name: &str) -> Value {
    json!({
        "identity": format!("x-demo:setting/{name}"),
        "kind": "setting",
        "spelling": name,
        "displayName": name,
        "setting": { "type": "integer" }
    })
}

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

fn display_name(spelling: &str) -> String {
    catalog()
        .into_iter()
        .find(|e| e["spelling"] == spelling)
        .and_then(|e| e["displayName"].as_str().map(str::to_string))
        .unwrap_or_else(|| spelling.to_string())
}
