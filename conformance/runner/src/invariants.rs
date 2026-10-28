//! Wire invariants checked on every actual response, independent of what a
//! fixture expects, because a fixture with provider-supplied capabilities
//! cannot know which optional fields the provider must or must not return.

use serde_json::Value;

#[derive(Default)]
pub(crate) struct Invariants {
    compile_inputs: bool,
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
                self.compile_inputs = minor.is_some_and(|minor| minor >= 7)
                    && result.pointer("/capabilities/compileInputs") == Some(&Value::Bool(true));
                Ok(())
            }
            Some("lpp/compile") => {
                let entry_based = request.pointer("/params/entry").is_some();
                check_compile_inputs(self.compile_inputs, entry_based, result)
            }
            _ => Ok(()),
        }
    }
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
