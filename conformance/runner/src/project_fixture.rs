//! Session-runtime project fixtures: materializing a scenario's
//! `projectFiles` into an isolated directory and substituting its URI for
//! `${PROJECT_URI}` in request and expected-response JSON. Consumed only by
//! `session`; the schema-side rules live in `scenario`.

use std::path::PathBuf;

use lpp_conformance_common::path_to_file_uri;
use serde_json::Value;

use crate::scenario::{Scenario, validate_project_path};

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
