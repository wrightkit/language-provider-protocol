//! LPP v1 conformance runner.
//!
//! Replays the versioned fixture scenarios in `conformance/fixtures/v1/`
//! against a provider binary and compares each response exactly.
//!
//! Usage:
//!
//! ```text
//! lpp-conformance-runner --validate-only [--fixtures <dir>] [--scope <all|protocol|semantics>]
//! lpp-conformance-runner --provider <path> [--fixtures <dir>] [--scope <all|protocol|semantics>]
//! ```
//!
//! * `--validate-only` checks the structure of every scenario file without
//!   spawning a provider.
//! * `--provider` runs the scenarios against the given provider binary. One
//!   fresh provider process is spawned per scenario, with the scenario's
//!   `providerArgs` (if any).
//! * `--scope protocol` runs only scenarios with `"scope": "protocol"`;
//!   `--scope semantics` runs only scenarios with `"scope": "semantics"`.
//!
//! A scenario ends by closing the provider's stdin; `expectExitCode` (default
//! 0) is checked against the provider's exit status.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

const STEP_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    /// Human-readable context for the scenario; informational only.
    #[allow(dead_code)]
    description: Option<String>,
    scope: String,
    #[serde(default)]
    provider_args: Vec<String>,
    #[serde(default)]
    project_files: Option<HashMap<String, String>>,
    steps: Vec<Step>,
    #[serde(default)]
    expect_exit_code: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// A normal JSON-RPC request, serialized and written as one line.
    request: Option<Value>,
    /// A raw line written verbatim (for malformed-message scenarios).
    #[serde(default)]
    raw_line: Option<String>,
    expect_response: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    fixtures: Option<String>,
    provider: Option<String>,
    validate_only: bool,
    scope: Option<String>,
}

fn main() {
    let args = parse_args();
    let fixtures_dir = PathBuf::from(
        args.fixtures
            .unwrap_or_else(|| "conformance/fixtures/v1".to_string()),
    );
    let scope_filter = args.scope.as_deref();

    let mut paths: Vec<PathBuf> = match std::fs::read_dir(&fixtures_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect(),
        Err(error) => {
            eprintln!(
                "lpp-conformance-runner: cannot read fixtures dir '{}': {error}",
                fixtures_dir.display()
            );
            std::process::exit(2);
        }
    };
    paths.sort();

    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<String> = Vec::new();

    for path in &paths {
        let scenario: Scenario = match read_scenario(path) {
            Ok(scenario) => scenario,
            Err(error) => {
                failed += 1;
                failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if let Some(scope) = scope_filter {
            if scope != "all" && scenario.scope != scope {
                println!("SKIP  {} (scope {})", scenario.name, scenario.scope);
                continue;
            }
        }
        if args.validate_only {
            println!("PASS  {} (structure)", scenario.name);
            passed += 1;
            continue;
        }
        match run_scenario(
            &scenario,
            args.provider.as_deref().expect("provider required"),
            passed + failed,
        ) {
            Ok(()) => {
                println!("PASS  {}", scenario.name);
                passed += 1;
            }
            Err(error) => {
                failed += 1;
                failures.push(format!("{}: {error}", scenario.name));
            }
        }
    }

    println!();
    println!("{passed} passed, {failed} failed");
    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failures:");
        for failure in &failures {
            eprintln!("- {failure}");
        }
        std::process::exit(1);
    }
    if paths.is_empty() {
        eprintln!(
            "lpp-conformance-runner: no fixture files found in '{}'",
            fixtures_dir.display()
        );
        std::process::exit(2);
    }
}

fn parse_args() -> Args {
    let mut args = Args {
        fixtures: None,
        provider: None,
        validate_only: false,
        scope: None,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fixtures" => args.fixtures = iter.next(),
            "--provider" => args.provider = iter.next(),
            "--validate-only" => args.validate_only = true,
            "--scope" => args.scope = iter.next(),
            other => {
                eprintln!("lpp-conformance-runner: unknown argument '{other}'");
                std::process::exit(2);
            }
        }
    }
    if !args.validate_only && args.provider.is_none() {
        eprintln!(
            "lpp-conformance-runner: --provider <path> is required unless --validate-only is given"
        );
        std::process::exit(2);
    }
    if let Some(scope) = &args.scope {
        if !["all", "protocol", "semantics"].contains(&scope.as_str()) {
            eprintln!("lpp-conformance-runner: --scope must be all, protocol, or semantics");
            std::process::exit(2);
        }
    }
    args
}

fn read_scenario(path: &Path) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read fixture: {e}"))?;
    let scenario: Scenario =
        serde_json::from_str(&text).map_err(|e| format!("fixture is not valid JSON: {e}"))?;
    validate_scenario(&scenario).map_err(|e| format!("invalid fixture: {e}"))?;
    Ok(scenario)
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
    let uses_project_uri = scenario.steps.iter().any(|step| {
        step.request.as_ref().is_some_and(contains_project_uri)
            || contains_project_uri(&step.expect_response)
    });
    if uses_project_uri && scenario.project_files.is_none() {
        return Err("'${PROJECT_URI}' requires 'projectFiles'".into());
    }
    if let Some(code) = scenario.expect_exit_code {
        if code < 0 {
            return Err("'expectExitCode' must be non-negative".into());
        }
    }
    for (i, step) in scenario.steps.iter().enumerate() {
        match (&step.request, &step.raw_line) {
            (Some(_), None) => {}
            (None, Some(_)) => {}
            _ => {
                return Err(format!(
                    "step {i}: step must have exactly one of 'request' or 'rawLine'"
                ));
            }
        }
        validate_expected(&step.expect_response).map_err(|e| format!("step {i}: {e}"))?;
        let request_id = step.request.as_ref().and_then(|r| r.get("id"));
        let response_id = step.expect_response.get("id");
        let id_ok = match (&step.request, &step.raw_line) {
            // Normal requests: the response id must echo the request id,
            // unless the scenario deliberately tests an id-less message
            // (notification, batch), in which case the response id is null.
            (Some(_), None) => match (request_id, response_id) {
                (Some(request_id), Some(response_id)) => request_id == response_id,
                (None, Some(Value::Null)) => true,
                _ => false,
            },
            // Raw lines carry no id; the expected response id must be null.
            (None, Some(_)) => matches!(response_id, Some(Value::Null)),
            _ => false,
        };
        if !id_ok {
            return Err(format!(
                "step {i}: expected response 'id' must match the request 'id'"
            ));
        }
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
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    if has_result == has_error {
        return Err("expected response must have exactly one of 'result' or 'error'".into());
    }
    Ok(())
}

fn run_scenario(scenario: &Scenario, provider: &str, scenario_index: usize) -> Result<(), String> {
    let project = materialize_project(scenario, scenario_index)?;
    let mut child = Command::new(provider)
        .args(&scenario.provider_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cannot spawn provider '{provider}': {e}"))?;

    let mut stdin = child.stdin.take().ok_or("provider stdin not available")?;
    let stdout = child.stdout.take().ok_or("provider stdout not available")?;

    let (response_tx, response_rx) = mpsc::channel();
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            if response_tx.send(line).is_err() {
                break;
            }
        }
    });

    for (i, step) in scenario.steps.iter().enumerate() {
        let request_line = match (&step.request, &step.raw_line) {
            (Some(request), None) => {
                let request = substitute_project_uri(request, project.as_ref());
                serde_json::to_string(&request).expect("request serializes")
            }
            (None, Some(raw)) => raw.clone(),
            _ => {
                return Err(format!(
                    "step {i}: step must have exactly one of 'request' or 'rawLine'"
                ));
            }
        };
        let mut writer = BufWriter::new(&mut stdin);
        if writeln!(writer, "{request_line}").is_err() {
            // The provider exited before reading the request.
            let status = wait_exit(&mut child);
            return Err(format!(
                "step {i}: provider exited before reading the request (exit status {status:?})"
            ));
        }
        writer
            .flush()
            .map_err(|e| format!("step {i}: cannot write request: {e}"))?;
        drop(writer);

        let response_line = match response_rx.recv_timeout(STEP_TIMEOUT) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let status = kill(&mut child);
                return Err(format!(
                    "step {i}: no response within {}s (provider exit status {status:?})",
                    STEP_TIMEOUT.as_secs()
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = wait_exit(&mut child);
                return Err(format!(
                    "step {i}: provider closed stdout without responding (exit status {status:?})"
                ));
            }
        };
        let actual: Value = serde_json::from_str(&response_line)
            .map_err(|e| format!("step {i}: response is not valid JSON: {e}"))?;
        let expected = substitute_project_uri(&step.expect_response, project.as_ref());
        if actual != expected {
            return Err(format!(
                "step {i}: response mismatch\n  expected: {}\n  actual:   {}",
                serde_json::to_string_pretty(&expected).expect("serializes"),
                serde_json::to_string_pretty(&actual).expect("serializes"),
            ));
        }
    }

    // End of scenario: close stdin and check the exit code.
    drop(stdin);
    let expected_exit = scenario.expect_exit_code.unwrap_or(0);
    let status = wait_exit(&mut child);
    let actual_exit = if let Some(code) = status.code() {
        code
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            eprintln!(
                "lpp-conformance-runner: provider '{}' terminated by signal {:?}",
                provider,
                status.signal()
            );
        }
        -1
    };
    if actual_exit != expected_exit {
        return Err(format!(
            "expected exit code {expected_exit}, got {actual_exit}"
        ));
    }
    let _ = reader_thread.join();
    Ok(())
}

struct ProjectFixture {
    directory: PathBuf,
    uri: String,
}

impl Drop for ProjectFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn materialize_project(
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
    let result = (|| {
        for (relative, contents) in files {
            validate_project_path(relative)?;
            let path = directory.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create project fixture parent: {error}"))?;
            }
            std::fs::write(&path, contents)
                .map_err(|error| format!("cannot write project fixture '{relative}': {error}"))?;
        }
        let directory = std::fs::canonicalize(&directory)
            .map_err(|error| format!("cannot canonicalize project fixture: {error}"))?;
        let uri = path_to_file_uri(&directory)?;
        Ok((directory, uri))
    })();
    let (directory, uri) = match result {
        Ok(result) => result,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&directory);
            return Err(error);
        }
    };
    Ok(Some(ProjectFixture { directory, uri }))
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

fn path_to_file_uri(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "project fixture path is not valid UTF-8".to_string())?;
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(uri)
}

fn substitute_project_uri(value: &Value, project: Option<&ProjectFixture>) -> Value {
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

fn wait_exit(child: &mut Child) -> std::process::ExitStatus {
    match child.try_wait() {
        Ok(Some(status)) => return status,
        Ok(None) => {}
        Err(_) => return std::process::ExitStatus::default(),
    }
    // Wait briefly, then kill.
    let deadline = std::time::Instant::now() + EXIT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status,
            Ok(None) => {}
            Err(_) => return std::process::ExitStatus::default(),
        }
        if std::time::Instant::now() >= deadline {
            eprintln!("lpp-conformance-runner: provider did not exit; killing it");
            let _ = child.kill();
            let _ = child.wait();
            return std::process::ExitStatus::default();
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn kill(child: &mut Child) -> Option<i32> {
    let _ = child.kill();
    let _ = child.wait();
    let status = child.try_wait().ok().flatten()?;
    status.code()
}
