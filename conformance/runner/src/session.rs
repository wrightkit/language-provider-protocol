//! Replaying one scenario against a spawned provider process: request lines
//! go to stdin, response lines are compared verbatim after JSON parsing, and
//! the process must exit with the expected status once stdin closes.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::project_fixture::{ProjectFixture, materialize_project, substitute_project_uri};
use crate::scenario::{Scenario, Step, StepRequest};

const STEP_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn run_scenario(
    scenario: &Scenario,
    provider: &str,
    scenario_index: usize,
) -> Result<(), String> {
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
    let (done_tx, done_rx) = mpsc::channel();
    let reader_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !line.trim().is_empty() && response_tx.send(line).is_err() {
                break;
            }
        }
        let _ = done_tx.send(());
    });

    for (i, step) in scenario.steps.iter().enumerate() {
        let request_line = step_request_line(step, project.as_ref());
        if writeln!(stdin, "{request_line}")
            .and_then(|_| stdin.flush())
            .is_err()
        {
            let status = wait_exit(&mut child, EXIT_TIMEOUT);
            return Err(format!(
                "step {i}: provider exited before reading the request (exit status {status:?})"
            ));
        }

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
                // stdout is closed, so no response can ever arrive; reap the
                // provider immediately instead of waiting out the timeout.
                let status = kill(&mut child);
                return Err(format!(
                    "step {i}: provider closed stdout without responding (exit status {status:?})"
                ));
            }
        };
        let actual: Value = match serde_json::from_str(&response_line) {
            Ok(actual) => actual,
            Err(e) => {
                kill(&mut child);
                return Err(format!("step {i}: response is not valid JSON: {e}"));
            }
        };
        let expected = substitute_project_uri(&step.expect_response, project.as_ref());
        if let Err(detail) = crate::matcher::matches(&expected, &actual) {
            kill(&mut child);
            return Err(format!(
                "step {i}: response mismatch at {detail}\n  expected: {}\n  actual:   {}",
                serde_json::to_string_pretty(&expected).expect("serializes"),
                serde_json::to_string_pretty(&actual).expect("serializes"),
            ));
        }
    }

    drop(stdin);
    let status = wait_exit(&mut child, EXIT_TIMEOUT);
    let actual_exit = status.code().unwrap_or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            eprintln!(
                "lpp-conformance-runner: provider '{provider}' terminated by signal {:?}",
                status.signal()
            );
        }
        -1
    });
    let expected_exit = scenario.expect_exit_code.unwrap_or(0);
    if actual_exit != expected_exit {
        return Err(format!(
            "expected exit code {expected_exit}, got {actual_exit}"
        ));
    }
    // Output the provider emitted past the last step must surface. Give the
    // reader a short grace period for already-buffered lines; a descendant
    // can keep the pipe open, so the wait stays bounded.
    const DRAIN_GRACE: Duration = Duration::from_millis(500);
    let _ = done_rx.recv_timeout(DRAIN_GRACE);
    let extra: Vec<String> = response_rx.try_iter().collect();
    if !extra.is_empty() {
        return Err(format!(
            "provider emitted {} unexpected line(s) after the last step: {extra:?}",
            extra.len()
        ));
    }
    // Do not join the reader: a provider descendant inheriting stdout keeps
    // the pipe open past the child's exit and a join could block forever.
    // The thread ends on EOF or a failed send, and dies with the process.
    drop(reader_thread);
    Ok(())
}

fn step_request_line(step: &Step, project: Option<&ProjectFixture>) -> String {
    match &step.request {
        StepRequest::Request(request) => {
            serde_json::to_string(&substitute_project_uri(request, project))
                .expect("request serializes")
        }
        // `${PROJECT_URI}` inside a raw line substitutes literally, matching
        // the request-object path.
        StepRequest::RawLine(raw) => raw.replace(
            "${PROJECT_URI}",
            project.map_or("", |project| project.uri.as_str()),
        ),
    }
}

/// Poll for exit until `timeout`, then kill and reap. Returns the exit
/// status, or a default when the child cannot be reaped.
fn wait_exit(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status,
            Err(_) => return std::process::ExitStatus::default(),
            Ok(None) if Instant::now() >= deadline => {
                eprintln!("lpp-conformance-runner: provider did not exit; killing it");
                return kill(child);
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Kill the provider and report its final exit status.
fn kill(child: &mut Child) -> std::process::ExitStatus {
    let _ = child.kill();
    child.wait().unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Scratch dir for fake-provider scripts under the repo's ignored
    /// `target/`; removed at test start and end so reruns stay clean.
    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/runner-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn scenario(script: &Path, extra_args: Vec<String>, steps: Vec<Step>) -> Scenario {
        Scenario {
            name: "test".into(),
            description: None,
            scope: "test".into(),
            session: None,
            provider_args: std::iter::once(script.to_str().unwrap().to_string())
                .chain(extra_args)
                .collect(),
            project_files: None,
            steps,
            expect_exit_code: Some(0),
        }
    }

    fn check_step(expect_response: Value) -> Step {
        Step {
            request: StepRequest::Request(
                json!({"jsonrpc":"2.0","id":1,"method":"lpp/check","params":{}}),
            ),
            expect_response,
        }
    }

    /// Run the script as `/bin/sh <script> <args…>` and assert the provider
    /// pid recorded in the pidfile is reaped after the scenario fails.
    fn assert_provider_reaped(script_body: &str) {
        let dir = scratch("reaped");
        let script = dir.join("provider.sh");
        let pidfile = dir.join("pid");
        fs::write(&script, script_body).unwrap();
        let scenario = scenario(
            &script,
            vec![pidfile.to_str().unwrap().to_string()],
            vec![check_step(json!({"jsonrpc":"2.0","id":1,"result":{"ok":true}}))],
        );
        assert!(run_scenario(&scenario, "/bin/sh", 0).is_err());
        let pid = fs::read_to_string(&pidfile).unwrap().trim().to_string();
        let alive = Command::new("kill")
            .args(["-0", &pid])
            .status()
            .is_ok_and(|s| s.success());
        // Clean up either way so a leaked sleeper does not linger.
        let _ = Command::new("kill").arg(&pid).status();
        fs::remove_dir_all(&dir).ok();
        assert!(!alive, "provider {pid} was not killed/reaped");
    }

    #[test]
    fn mismatched_response_kills_provider() {
        // A response that fails the matcher must not leave the provider
        // running.
        assert_provider_reaped(
            "#!/bin/sh\necho $$ > \"$1\"\necho '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}'\nexec sleep 30\n",
        );
    }

    #[test]
    fn unparseable_response_kills_provider() {
        // A non-JSON line must not leave the provider running either.
        assert_provider_reaped("#!/bin/sh\necho $$ > \"$1\"\necho 'not-json'\nexec sleep 30\n");
    }

    #[test]
    fn exit_is_not_blocked_by_descendant_holding_stdout() {
        // The provider answers and exits while a spawned descendant keeps
        // stdout open; the runner must not block joining the reader thread.
        let dir = scratch("grandchild");
        let script = dir.join("provider.sh");
        let pidfile = dir.join("pid");
        fs::write(
            &script,
            "#!/bin/sh\nsleep 8 &\necho $! > \"$1\"\necho '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}'\nexit 0\n",
        )
        .unwrap();
        let scenario = scenario(
            &script,
            vec![pidfile.to_str().unwrap().to_string()],
            vec![check_step(json!({"jsonrpc":"2.0","id":1,"result":{}}))],
        );
        let start = Instant::now();
        run_scenario(&scenario, "/bin/sh", 0).expect("scenario passes");
        let elapsed = start.elapsed();
        let pid = fs::read_to_string(&pidfile).unwrap().trim().to_string();
        let _ = Command::new("kill").arg(&pid).status();
        fs::remove_dir_all(&dir).ok();
        assert!(
            elapsed < Duration::from_secs(4),
            "blocked {elapsed:?} on a descendant holding stdout"
        );
    }

    #[test]
    fn output_past_the_last_step_fails() {
        // A provider that emits an extra line after answering must not pass.
        let dir = scratch("extra");
        let script = dir.join("provider.sh");
        fs::write(
            &script,
            "#!/bin/sh\necho '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}'\necho '{\"extra\":true}'\nexit 0\n",
        )
        .unwrap();
        let scenario = scenario(
            &script,
            vec![],
            vec![check_step(json!({"jsonrpc":"2.0","id":1,"result":{}}))],
        );
        let error = run_scenario(&scenario, "/bin/sh", 0).expect_err("extra output fails");
        fs::remove_dir_all(&dir).ok();
        assert!(error.contains("unexpected line"), "got: {error}");
    }

    #[test]
    fn raw_line_substitutes_project_uri() {
        // `${PROJECT_URI}` inside a rawLine must be replaced like it is in
        // request objects; the provider only answers when no placeholder
        // survives on the wire.
        let dir = scratch("rawline");
        let script = dir.join("provider.sh");
        fs::write(
            &script,
            "#!/bin/sh\nread line\ncase \"$line\" in\n*'${PROJECT_URI}'*) echo '{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32600,\"message\":\"placeholder survived\"}}' ;;\n*) echo '{\"jsonrpc\":\"2.0\",\"id\":null,\"result\":{}}' ;;\nesac\nexit 0\n",
        )
        .unwrap();
        let mut scenario = scenario(
            &script,
            vec![],
            vec![Step {
                request: StepRequest::RawLine(
                    r#"{"jsonrpc":"2.0","id":1,"method":"lpp/check","params":{"entry":{"uri":"${PROJECT_URI}","languageId":"x-demo-lang","version":1}}}"#.to_string(),
                ),
                expect_response: json!({"jsonrpc":"2.0","id":null,"result":{}}),
            }],
        );
        scenario.project_files =
            Some(std::collections::HashMap::from([("entry.xdl".to_string(), String::new())]));
        run_scenario(&scenario, "/bin/sh", 0).expect("placeholder substituted");
        fs::remove_dir_all(&dir).ok();
    }
}
