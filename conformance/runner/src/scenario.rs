//! Scenario files: the fixture schema (`*.json` under `fixtures/v1/`) and the
//! structural validation the runner performs before replaying anything.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lpp_conformance_common::path_to_file_uri;
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
#[serde(rename_all = "camelCase")]
pub(crate) struct Step {
    pub request: Option<Value>,
    #[serde(default)]
    pub raw_line: Option<String>,
    pub expect_response: Value,
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
            let method = step
                .request
                .as_ref()
                .and_then(|r| r.get("method"))
                .and_then(Value::as_str)
                .or_else(|| {
                    let line = step.raw_line.as_deref()?;
                    ["lpp/initialize", "lpp/shutdown"]
                        .into_iter()
                        .find(|m| line.contains(m))
                });
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
        if let Some(request) = step.request.as_mut().and_then(Value::as_object_mut) {
            request.entry("jsonrpc").or_insert_with(|| json!("2.0"));
        }
        if let Some(response) = step.expect_response.as_object_mut() {
            response.entry("jsonrpc").or_insert_with(|| json!("2.0"));
            if !response.contains_key("id") {
                let id = step
                    .request
                    .as_ref()
                    .and_then(|r| r.get("id"))
                    .cloned()
                    .unwrap_or(Value::Null);
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
        let request_id = step.request.as_ref().and_then(|r| r.get("id"));
        let response_id = step.expect_response.get("id");
        // Normal requests: the response id echoes the request id, unless the
        // scenario deliberately sends an id-less message (notification,
        // batch), in which case the response id is null. Raw lines carry no
        // id, so the expected response id must be null.
        let id_ok = match (&step.request, &step.raw_line) {
            (Some(_), None) => match (request_id, response_id) {
                (Some(request_id), Some(response_id)) => request_id == response_id,
                (None, Some(Value::Null)) => true,
                _ => false,
            },
            (None, Some(_)) => matches!(response_id, Some(Value::Null)),
            _ => {
                return Err(format!(
                    "step {i}: step must have exactly one of 'request' or 'rawLine'"
                ));
            }
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
        step.request.as_ref().is_some_and(contains_project_uri)
            || contains_project_uri(&step.expect_response)
    })
}

fn validate_project_path(relative: &str) -> Result<(), String> {
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

/// A materialized `projectFiles` tree; removed when dropped.
pub(crate) struct ProjectFixture {
    directory: PathBuf,
    pub uri: String,
}

impl Drop for ProjectFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Write `projectFiles` into an isolated directory under `target/`.
/// `Ok(None)` when the scenario has no project files.
pub(crate) fn materialize_project(
    scenario: &Scenario,
    scenario_index: usize,
) -> Result<Option<ProjectFixture>, String> {
    let Some(files) = &scenario.project_files else {
        return Ok(None);
    };
    let root = PathBuf::from("target/lpp-conformance-projects");
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("cannot create project fixture root: {error}"))?;
    let directory = root.join(format!("{}-{}", std::process::id(), scenario_index));
    std::fs::create_dir(&directory)
        .map_err(|error| format!("cannot create project fixture directory: {error}"))?;

    // From here on the fixture owns the directory; any error cleans it up.
    let mut fixture = ProjectFixture {
        directory,
        uri: String::new(),
    };
    for (relative, contents) in files {
        validate_project_path(relative)?;
        let path = fixture.directory.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create project fixture parent: {error}"))?;
        }
        std::fs::write(&path, contents)
            .map_err(|error| format!("cannot write project fixture '{relative}': {error}"))?;
    }
    fixture.directory = std::fs::canonicalize(&fixture.directory)
        .map_err(|error| format!("cannot canonicalize project fixture: {error}"))?;
    fixture.uri = path_to_file_uri(&fixture.directory)
        .ok_or_else(|| "project fixture path is not valid UTF-8".to_string())?;
    Ok(Some(fixture))
}

/// Recursively replace `${PROJECT_URI}` in fixture JSON.
pub(crate) fn substitute_project_uri(value: &Value, project: Option<&ProjectFixture>) -> Value {
    match value {
        Value::String(text) => {
            let replacement = project.map_or_else(String::new, |project| project.uri.clone());
            Value::String(text.replace("${PROJECT_URI}", &replacement))
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| substitute_project_uri(value, project))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), substitute_project_uri(value, project)))
                .collect(),
        ),
        value => value.clone(),
    }
}
