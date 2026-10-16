//! CLI-level regressions: flag values must not swallow the next flag, and
//! fixture discovery must ignore non-files.

use std::path::PathBuf;
use std::process::Command;

fn runner() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lpp-conformance-runner"))
}

/// Scratch dir under the repo's ignored `target/`; removed on entry so
/// reruns stay clean.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/cli-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn flag_without_value_is_an_error_not_the_next_flag() {
    let out = runner()
        .args(["--provider", "--validate-only"])
        .output()
        .expect("run runner");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
    assert!(stderr.contains("--provider requires a value"), "{stderr}");
}

#[test]
fn fixtures_dir_skips_json_directories() {
    // A directory named `*.json` is not a fixture; discovery must not feed
    // it to the scenario parser.
    let dir = scratch("fixtures");
    std::fs::write(
        dir.join("ok.json"),
        r#"{"name":"ok","scope":"protocol","steps":[{"request":{"id":1,"method":"lpp/check","params":{}},"expectResponse":{"result":null}}]}"#,
    )
    .unwrap();
    std::fs::create_dir(dir.join("stray.json")).unwrap();
    let out = runner()
        .args(["--validate-only", "--fixtures"])
        .arg(&dir)
        .output()
        .expect("run runner");
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    std::fs::remove_dir_all(&dir).ok();
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("PASS  ok"), "{stdout}");
    assert!(!stdout.contains("stray"), "{stdout}");
}
