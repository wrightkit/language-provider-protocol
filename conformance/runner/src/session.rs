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
    let reader_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !line.trim().is_empty() && response_tx.send(line).is_err() {
                break;
            }
        }
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
                let status = wait_exit(&mut child, EXIT_TIMEOUT);
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
    let _ = reader_thread.join();
    Ok(())
}

fn step_request_line(step: &Step, project: Option<&ProjectFixture>) -> String {
    match &step.request {
        StepRequest::Request(request) => {
            serde_json::to_string(&substitute_project_uri(request, project))
                .expect("request serializes")
        }
        StepRequest::RawLine(raw) => raw.clone(),
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
