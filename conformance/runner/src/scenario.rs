//! Scenario files: the fixture schema (`*.json` under `fixtures/v1/`) and the
//! structural validation the runner performs before replaying anything.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Scenario {
    pub name: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub scope: String,
    /// Wraps `steps` in the standard initialize/shutdown handshake for the
    /// given protocol version, spliced from `sessions/<version>.json`.
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub provider_args: Vec<String>,
    #[serde(default)]
    pub project_files: Option<HashMap<String, String>>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub expect_exit_code: Option<i32>,
}

#[derive(Deserialize)]
#[serde(try_from = "StepWire")]
pub(crate) struct Step {
    /// The one request form the step sends.
    pub request: StepRequest,
    pub expect_response: Value,
}

/// A step's outbound payload: either a JSON-RPC `request` object or a
/// `rawLine` sent verbatim — never both, never neither.
pub(crate) enum StepRequest {
    Request(Value),
    RawLine(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepWire {
    request: Option<Value>,
    raw_line: Option<String>,
    expect_response: Value,
}

impl TryFrom<StepWire> for Step {
    type Error = String;

    fn try_from(wire: StepWire) -> Result<Self, Self::Error> {
        let request = match (wire.request, wire.raw_line) {
            (Some(request), None) => StepRequest::Request(request),
            (None, Some(raw)) => StepRequest::RawLine(raw),
            _ => {
                return Err("step must have exactly one of 'request' or 'rawLine'".into());
            }
        };
        Ok(Step {
            request,
            expect_response: wire.expect_response,
        })
    }
}

/// The literal handshake a `session` fixture inherits: the exact
/// initialize/shutdown steps live in `sessions/<version>.json` next to the
/// fixtures, keeping expected responses as data rather than runner logic.
#[derive(Deserialize)]
struct SessionTemplate {
    initialize: Step,
    shutdown: Step,
}

pub(crate) fn read_scenario(path: &Path) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read fixture: {e}"))?;
    let mut scenario: Scenario =
        serde_json::from_str(&text).map_err(|e| format!("fixture is not valid JSON: {e}"))?;
    if let Some(version) = &scenario.session {
        // The handshake comes from the template; stepping it twice is a
        // session-behavior test and must stay explicit.
        for (i, step) in scenario.steps.iter().enumerate() {
            let method = match &step.request {
                StepRequest::Request(request) => request.get("method").and_then(Value::as_str),
                StepRequest::RawLine(line) => ["lpp/initialize", "lpp/shutdown"]
                    .into_iter()
                    .find(|m| line.contains(m)),
            };
            if matches!(method, Some("lpp/initialize" | "lpp/shutdown")) {
                return Err(format!(
                    "invalid fixture: step {i}: a 'session' scenario must not step {method:?}"
                ));
            }
        }
        let template = read_session_template(path, version)?;
        let mut steps = vec![template.initialize];
        steps.append(&mut scenario.steps);
        steps.push(template.shutdown);
        scenario.steps = steps;
    }
    apply_envelope_defaults(&mut scenario.steps);
    validate_scenario(&scenario).map_err(|e| format!("invalid fixture: {e}"))?;
    Ok(scenario)
}

/// Wire-envelope defaults so fixtures state only what varies: `jsonrpc` is
/// `"2.0"` on both sides, and an absent `expectResponse.id` expects the
/// request's id (null for `rawLine` steps).
fn apply_envelope_defaults(steps: &mut [Step]) {
    for step in steps {
        if let StepRequest::Request(request) = &mut step.request {
            if let Some(request) = request.as_object_mut() {
                request.entry("jsonrpc").or_insert_with(|| json!("2.0"));
            }
        }
        if let Some(response) = step.expect_response.as_object_mut() {
            response.entry("jsonrpc").or_insert_with(|| json!("2.0"));
            if !response.contains_key("id") {
                let id = match &step.request {
                    StepRequest::Request(request) => {
                        request.get("id").cloned().unwrap_or(Value::Null)
                    }
                    StepRequest::RawLine(_) => Value::Null,
                };
                response.insert("id".to_string(), id);
            }
        }
    }
}

fn read_session_template(path: &Path, version: &str) -> Result<SessionTemplate, String> {
    if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return Err(format!("session '{version}': invalid session name"));
    }
    let file = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sessions")
        .join(format!("{version}.json"));
    let text = std::fs::read_to_string(&file)
        .map_err(|e| format!("session '{version}': cannot read '{}': {e}", file.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("session '{version}': invalid template: {e}"))
}

fn validate_scenario(scenario: &Scenario) -> Result<(), String> {
    if scenario.name.is_empty() {
        return Err("missing or empty 'name'".into());
    }
    if scenario.scope != "protocol" && scenario.scope != "semantics" {
        return Err("'scope' must be 'protocol' or 'semantics'".into());
    }
    if scenario.steps.is_empty() {
        return Err("'steps' must be non-empty".into());
    }
    if let Some(files) = &scenario.project_files {
        for relative in files.keys() {
            validate_project_path(relative)?;
        }
    }
    if uses_project_uri(scenario) && scenario.project_files.is_none() {
        return Err("'${PROJECT_URI}' requires 'projectFiles'".into());
    }
    if let Some(code) = scenario.expect_exit_code {
        if code < 0 {
            return Err("'expectExitCode' must be non-negative".into());
        }
    }
    for (i, step) in scenario.steps.iter().enumerate() {
        let response_id = step.expect_response.get("id");
        // Normal requests: the response id echoes the request id, unless the
        // scenario deliberately sends an id-less message (notification,
        // batch), in which case the response id is null. Raw lines carry no
        // id, so the expected response id must be null.
        let id_ok = match &step.request {
            StepRequest::Request(request) => match (request.get("id"), response_id) {
                (Some(request_id), Some(response_id)) => request_id == response_id,
                (None, Some(Value::Null)) => true,
                _ => false,
            },
            StepRequest::RawLine(_) => matches!(response_id, Some(Value::Null)),
        };
        if !id_ok {
            return Err(format!(
                "step {i}: expected response 'id' must match the request 'id'"
            ));
        }
        validate_expected(&step.expect_response).map_err(|e| format!("step {i}: {e}"))?;
    }
    Ok(())
}

fn validate_expected(response: &Value) -> Result<(), String> {
    let object = response
        .as_object()
        .ok_or("expected response must be an object")?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err("expected response 'jsonrpc' must be \"2.0\"".into());
    }
    if !object.contains_key("id") {
        return Err("expected response must have an 'id'".into());
    }
    if object.contains_key("result") == object.contains_key("error") {
        return Err("expected response must have exactly one of 'result' or 'error'".into());
    }
    Ok(())
}

fn uses_project_uri(scenario: &Scenario) -> bool {
    scenario.steps.iter().any(|step| {
        let request_uses_uri = match &step.request {
            StepRequest::Request(request) => contains_project_uri(request),
            StepRequest::RawLine(_) => false,
        };
        request_uses_uri || contains_project_uri(&step.expect_response)
    })
}

pub(crate) fn validate_project_path(relative: &str) -> Result<(), String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err(format!("invalid project fixture path '{relative}'"));
    }
    Ok(())
}

fn contains_project_uri(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("${PROJECT_URI}"),
        Value::Array(values) => values.iter().any(contains_project_uri),
        Value::Object(object) => object.values().any(contains_project_uri),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema rejects a step carrying both request forms — all shipped
    /// fixtures are valid, so only these unit tests exercise the rule.
    #[test]
    fn step_rejects_request_and_raw_line_together() {
        let step = json!({
            "request": {"id": 1, "method": "lpp/check", "params": {}},
            "rawLine": "{}",
            "expectResponse": {"result": null},
        });
        let error = serde_json::from_value::<Step>(step)
            .err()
            .expect("a step with both request forms must fail");
        assert!(
            error
                .to_string()
                .contains("exactly one of 'request' or 'rawLine'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn step_rejects_no_request_form() {
        let step = json!({
            "expectResponse": {"result": null},
        });
        let error = serde_json::from_value::<Step>(step)
            .err()
            .expect("a step with no request form must fail");
        assert!(
            error
                .to_string()
                .contains("exactly one of 'request' or 'rawLine'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn step_accepts_each_request_form() {
        let request_step: Step = serde_json::from_value(json!({
            "request": {"id": 1, "method": "lpp/check", "params": {}},
            "expectResponse": {"result": null},
        }))
        .expect("request step parses");
        assert!(matches!(request_step.request, StepRequest::Request(_)));

        let raw_step: Step = serde_json::from_value(json!({
            "rawLine": "not json",
            "expectResponse": {"result": null},
        }))
        .expect("rawLine step parses");
        assert!(matches!(raw_step.request, StepRequest::RawLine(_)));
    }
}
