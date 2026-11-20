use serde_json::Value;

#[derive(Default)]
pub(crate) struct Invariants {
    compile_inputs: bool,
    minor: Option<u32>,
}

impl Invariants {
    pub(crate) fn check(&mut self, request: &Value, response: &Value) -> Result<(), String> {
        let Some(result) = response.get("result") else {
            return Ok(());
        };
        match request.get("method").and_then(Value::as_str) {
            Some("lpp/initialize") => {
                let minor = result
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .and_then(|version| version.strip_prefix("1."))
                    .and_then(|minor| minor.parse::<u32>().ok());
                self.minor = minor;
                self.compile_inputs = minor.is_some_and(|minor| minor >= 7)
                    && result.pointer("/capabilities/compileInputs") == Some(&Value::Bool(true));
                Ok(())
            }
            Some("lpp/compile") => {
                let entry_based = request.pointer("/params/entry").is_some();
                check_compile_inputs(self.compile_inputs, entry_based, result)
            }
            Some("lpp/lookup") => check_lookup(self.minor.is_some_and(|minor| minor >= 8), result),
            _ => Ok(()),
        }
    }
}

fn check_lookup(negotiated: bool, result: &Value) -> Result<(), String> {
    let entries = result.get("entries").and_then(Value::as_array);
    for entry in entries.into_iter().flatten() {
        let described = entry.get("description").into_iter().chain(
            entry
                .pointer("/callable/parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|parameter| parameter.get("description")),
        );
        for description in described {
            if !negotiated {
                return Err(
                    "lookup result includes 'description' in a session before LPP 1.8".into(),
                );
            }
            if description.as_str().is_none_or(str::is_empty) {
                return Err("lookup 'description' must be a non-empty string".into());
            }
        }
        let Some(aliases) = entry.get("aliases") else {
            continue;
        };
        if !negotiated {
            return Err("lookup result includes 'aliases' in a session before LPP 1.8".into());
        }
        let aliases = aliases
            .as_array()
            .and_then(|aliases| {
                aliases
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<Vec<_>>>()
            })
            .filter(|aliases| !aliases.is_empty() && aliases.iter().all(|alias| !alias.is_empty()))
            .ok_or("lookup 'aliases' must be a non-empty array of non-empty strings")?;
        if aliases
            .iter()
            .any(|alias| Some(*alias) == entry["spelling"].as_str())
        {
            return Err("lookup 'aliases' must not contain the entry's 'spelling'".into());
        }
    }
    Ok(())
}

fn check_compile_inputs(negotiated: bool, entry_based: bool, result: &Value) -> Result<(), String> {
    let Some(inputs) = result.get("compileInputs") else {
        if negotiated && entry_based {
            return Err("entry-based compile result omits 'compileInputs' although the session negotiated the 'compileInputs' capability".into());
        }
        return Ok(());
    };
    if !negotiated {
        return Err("compile result includes 'compileInputs' without the negotiated 'compileInputs' capability".into());
    }
    if !entry_based {
        return Err("'compileInputs' is defined only for entry-based compile results".into());
    }
    let uris = inputs
        .as_array()
        .and_then(|uris| uris.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
        .filter(|uris| uris.iter().all(|uri| uri.starts_with("file:///")))
        .ok_or("'compileInputs' must be an array of absolute file URIs")?;
    if !uris.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("'compileInputs' must be sorted and free of duplicates".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn initialize(version: &str, compile_inputs: bool) -> Invariants {
        let mut invariants = Invariants::default();
        invariants
            .check(
                &json!({"method": "lpp/initialize"}),
                &json!({"result": {"protocolVersion": version, "capabilities": {"compileInputs": compile_inputs}}}),
            )
            .expect("initialize carries no compile invariant");
        invariants
    }

    fn compile(invariants: &mut Invariants, params: Value, result: Value) -> Result<(), String> {
        invariants.check(
            &json!({"method": "lpp/compile", "params": params}),
            &json!({"result": result}),
        )
    }

    fn entry() -> Value {
        json!({"entry": {"uri": "file:///p", "languageId": "x", "version": 1}})
    }

    fn with_inputs(inputs: Value) -> Value {
        json!({"diagnostics": [], "artifact": null, "compileInputs": inputs})
    }

    fn lookup(invariants: &mut Invariants, entry: Value) -> Result<(), String> {
        invariants.check(
            &json!({"method": "lpp/lookup"}),
            &json!({"result": {"entries": [entry]}}),
        )
    }

    fn entry_with(extra: Value) -> Value {
        let mut entry = json!({"identity": "i", "kind": "k", "spelling": "s", "displayName": "d"});
        entry
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        entry
    }

    #[test]
    fn lookup_accepts_description_and_aliases_in_1_8() {
        let mut invariants = initialize("1.8", false);
        let entry = entry_with(json!({
            "description": "d", "aliases": ["a"],
            "callable": {"parameters": [{"name": "p", "type": "t", "required": true, "description": "x"}]}
        }));
        assert_eq!(lookup(&mut invariants, entry), Ok(()));
    }

    #[test]
    fn lookup_rejects_the_fields_before_1_8() {
        let mut invariants = initialize("1.7", false);
        assert!(lookup(&mut invariants, entry_with(json!({"description": "d"}))).is_err());
        assert!(lookup(&mut invariants, entry_with(json!({"aliases": ["a"]}))).is_err());
        let parameter = json!({"callable": {"parameters": [{"name": "p", "type": "t", "required": true, "description": "x"}]}});
        assert!(lookup(&mut invariants, entry_with(parameter)).is_err());
    }

    #[test]
    fn lookup_rejects_malformed_description_and_aliases() {
        let mut invariants = initialize("1.8", false);
        for bad in [
            json!({"description": ""}),
            json!({"description": 3}),
            json!({"aliases": []}),
            json!({"aliases": [""]}),
            json!({"aliases": ["a", 1]}),
            json!({"aliases": ["s"]}),
        ] {
            assert!(
                lookup(&mut invariants, entry_with(bad.clone())).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_inputs_without_the_capability() {
        for mut invariants in [
            Invariants::default(),
            initialize("1.7", false),
            initialize("1.6", true),
        ] {
            let error = compile(&mut invariants, entry(), with_inputs(json!(["file:///p"])))
                .expect_err("field without capability");
            assert!(error.contains("without the negotiated"), "got: {error}");
        }
    }

    #[test]
    fn accepts_inputs_with_the_capability() {
        let mut invariants = initialize("1.7", true);
        assert!(
            compile(
                &mut invariants,
                entry(),
                with_inputs(json!(["file:///p", "file:///p/a.xdl", "file:///p/b.xdl"]))
            )
            .is_ok()
        );
        assert!(compile(&mut invariants, entry(), with_inputs(json!([]))).is_ok());
    }

    #[test]
    fn accepts_omission_without_the_capability() {
        let mut invariants = initialize("1.7", false);
        assert!(
            compile(
                &mut invariants,
                entry(),
                json!({"diagnostics": [], "artifact": null})
            )
            .is_ok()
        );
    }

    #[test]
    fn requires_inputs_for_entry_compiles_once_negotiated() {
        let mut invariants = initialize("1.7", true);
        assert!(
            compile(
                &mut invariants,
                entry(),
                json!({"diagnostics": [], "artifact": null})
            )
            .is_err()
        );
        let documents = json!({"documents": {}});
        assert!(
            compile(
                &mut invariants,
                documents.clone(),
                json!({"diagnostics": [], "artifact": null})
            )
            .is_ok()
        );
        assert!(compile(&mut invariants, documents, with_inputs(json!([]))).is_err());
    }

    #[test]
    fn rejects_malformed_inputs() {
        let mut invariants = initialize("1.7", true);
        for inputs in [
            json!("file:///p"),
            json!([1]),
            json!(["relative/a.xdl"]),
            json!(["file:///p/b.xdl", "file:///p/a.xdl"]),
            json!(["file:///p/a.xdl", "file:///p/a.xdl"]),
        ] {
            assert!(
                compile(&mut invariants, entry(), with_inputs(inputs.clone())).is_err(),
                "should reject {inputs}"
            );
        }
    }

    #[test]
    fn error_responses_are_not_checked() {
        let mut invariants = Invariants::default();
        assert!(
            invariants
                .check(
                    &json!({"method": "lpp/compile", "params": entry()}),
                    &json!({"error": {}})
                )
                .is_ok()
        );
    }
}
