//! Stdio-framing regressions: malformed bytes on the wire must not crash the
//! provider, and EOF must end the session cleanly.

use std::io::Write;
use std::process::{Command, Stdio};

fn provider() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lpp-mock-provider"))
}

/// Feed `input` to a fresh provider, then close stdin and collect the
/// response messages.
fn run_session(input: &[u8]) -> Vec<serde_json::Value> {
    run_session_args(&[], input)
}

/// `run_session` with extra provider command-line arguments.
fn run_session_args(args: &[&str], input: &[u8]) -> Vec<serde_json::Value> {
    let mut child = provider()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn provider");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write input");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "provider exited {:?}", out.status);
    String::from_utf8(out.stdout)
        .expect("utf8 stdout")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json response"))
        .collect()
}

const INIT: &str =
    r#"{"jsonrpc":"2.0","id":1,"method":"lpp/initialize","params":{"protocolVersion":"1.0"}}"#;

#[test]
fn non_utf8_line_gets_parse_error_and_session_continues() {
    let mut child = provider()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn provider");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(b"\xff\n").expect("write invalid bytes");
        // A well-formed request after the bad line: the session must survive.
        stdin
            .write_all(
                br#"{"jsonrpc":"2.0","id":1,"method":"lpp/initialize","params":{"protocolVersion":"1.0"}}"#
                .as_slice(),
            )
            .and_then(|()| stdin.write_all(b"\n"))
            .expect("write initialize");
    }
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "provider exited {:?}", out.status);
    let text = String::from_utf8(out.stdout).expect("utf8 stdout");
    let mut lines = text.lines();
    let parse_error: serde_json::Value =
        serde_json::from_str(lines.next().expect("first line")).expect("json response");
    assert_eq!(parse_error["error"]["code"], -32700);
    assert_eq!(parse_error["id"], serde_json::Value::Null);
    let init: serde_json::Value =
        serde_json::from_str(lines.next().expect("second line")).expect("json response");
    assert_eq!(init["id"], 1);
    assert!(init["result"]["protocolVersion"].is_string());
}

#[test]
fn rename_refuses_when_a_document_has_errors() {
    // A document whose parse produced error diagnostics cannot be renamed
    // atomically — spec section 15.2 requires covering every reference in
    // every received document — so the provider must refuse rather than
    // emit partial edits.
    let clean = "puzzle c {\n  target = 2\n  start = 1\n  ops {\n    a: x => x + 1\n  }\n  solution = [ a ]\n}";
    let dup = "puzzle p {\n  target = 1\n  start = 0\n  ops {\n    a: x => x + 1\n    a: x => x + 2\n  }\n  solution = [ a ]\n}";
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"lpp/rename","params":{{"documents":{{"c":{{"uri":"file:///c.xdl","languageId":"x-demo-lang","version":1,"text":{clean}}},"d":{{"uri":"file:///d.xdl","languageId":"x-demo-lang","version":1,"text":{dup}}}}},"positionDocumentUri":"c","position":{{"line":4,"character":4}},"newName":"b"}}}}"#,
        clean = serde_json::to_string(clean).unwrap(),
        dup = serde_json::to_string(dup).unwrap(),
    );
    let responses = run_session(format!("{INIT}\n{request}\n").as_bytes());
    let lpp = &responses[1]["error"]["data"]["lpp"];
    assert_eq!(lpp["kind"], "invalidDocument", "expected refusal, got {responses:?}");
}

/// Scratch directory under the repo's ignored `target/` for project-loader
/// tests. Removed at test start and end so reruns stay clean.
fn project_dir(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/project-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create project dir");
    root
}

const VALID: &str =
    "puzzle p {\n  target = 1\n  start = 0\n  ops {\n    a: x => x + 1\n  }\n  solution = [ a ]\n}";

fn check_directory(root: &std::path::Path) -> serde_json::Value {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"lpp/initialize","params":{"protocolVersion":"1.6"}}"#;
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"lpp/check","params":{{"entry":{{"uri":"file://{}","languageId":"x-demo-lang","version":1,"kind":"directory"}}}}}}"#,
        root.display()
    );
    // Directory entries need LPP 1.2+, so pin the provider's version.
    let responses = run_session_args(
        &["--protocol-version", "1.6"],
        format!("{init}\n{request}\n").as_bytes(),
    );
    responses.into_iter().nth(1).expect("check response")
}

#[test]
fn project_loader_ignores_xdl_directories() {
    // A directory named `*.xdl` is not a source file; loading must skip it
    // rather than fail the whole request trying to read it.
    let root = project_dir("xdl-dir");
    std::fs::write(root.join("entry.xdl"), VALID).unwrap();
    std::fs::create_dir(root.join("junk.xdl")).unwrap();
    let response = check_directory(&root);
    std::fs::remove_dir_all(&root).ok();
    assert!(
        response["result"].is_object(),
        "expected check result, got {response}"
    );
}

#[cfg(unix)]
#[test]
fn project_load_failure_names_the_broken_source_uri() {
    // A dangling symlink canonicalizes to nothing: the failure's
    // `details.uri` must name the file that could not be loaded.
    let root = project_dir("dangling");
    std::fs::write(root.join("entry.xdl"), VALID).unwrap();
    std::os::unix::fs::symlink("missing-target.xdl", root.join("broken.xdl")).unwrap();
    let response = check_directory(&root);
    std::fs::remove_dir_all(&root).ok();
    let lpp = &response["error"]["data"]["lpp"];
    assert_eq!(lpp["kind"], "projectLoadFailed", "got {response}");
    let uri = lpp["details"]["uri"].as_str().unwrap_or_default();
    assert!(uri.ends_with("/broken.xdl"), "details.uri: {uri}");
}
