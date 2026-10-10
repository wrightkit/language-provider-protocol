//! The canonical-id to `x-demo-lang` spelling table served by
//! `lpp/resolveIds`.
//!
//! The ids are the Workshop ids the protocol fixtures use. The demo language
//! maps only a few of them, with spellings unlike OverPy's, so a client cannot
//! pass by assuming spelling equals id. Every other id is unmapped and is
//! omitted from the result.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

const CALLS: &[(&str, &str)] = &[("applyImpulse", "shove"), ("setStatusEffect", "afflict")];

const ENUM_MEMBERS: &[(&str, &str, &str)] = &[("Status", "STUNNED", "stunned")];

/// The result for the requested ids: only mapped ids, with `enums` grouped by
/// domain and a domain omitted when none of its members map.
pub(crate) fn resolve_ids(calls: &[String], enums: &BTreeMap<String, Vec<String>>) -> Value {
    let calls: Map<String, Value> = calls
        .iter()
        .filter_map(|id| {
            let (_, spelling) = CALLS.iter().find(|(known, _)| known == id)?;
            Some((id.clone(), json!(spelling)))
        })
        .collect();
    let mut domains = Map::new();
    for (domain, members) in enums {
        let mapped: Map<String, Value> = members
            .iter()
            .filter_map(|member| {
                let (_, _, spelling) = ENUM_MEMBERS
                    .iter()
                    .find(|(d, m, _)| d == domain && m == member)?;
                Some((member.clone(), json!(spelling)))
            })
            .collect();
        if !mapped.is_empty() {
            domains.insert(domain.clone(), Value::Object(mapped));
        }
    }
    json!({ "calls": calls, "enums": domains })
}
