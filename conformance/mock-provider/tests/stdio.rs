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
    let mut child = provider()
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
