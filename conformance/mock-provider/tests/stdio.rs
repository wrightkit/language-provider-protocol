//! Stdio-framing regressions: malformed bytes on the wire must not crash the
//! provider, and EOF must end the session cleanly.

use std::io::Write;
use std::process::{Command, Stdio};

fn provider() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lpp-mock-provider"))
}

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
