//! `lpp-conformance-runner`: replays LPP v1 fixtures against a provider
//! binary. `scenario` owns the fixture schema and structural validation,
//! `project_fixture` owns the materialized `projectFiles` tree and
//! `${PROJECT_URI}` substitution, and `session` owns the spawned provider
//! exchange. Fixtures must stay deterministic and self-contained.

mod project_fixture;
mod scenario;
mod session;

use std::path::PathBuf;

use serde::Deserialize;

use scenario::read_scenario;
use session::run_scenario;

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
            usage_error(&format!(
                "cannot read fixtures dir '{}': {error}",
                fixtures_dir.display()
            ));
        }
    };
    paths.sort();
    if paths.is_empty() {
        usage_error(&format!(
            "no fixture files found in '{}'",
            fixtures_dir.display()
        ));
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<String> = Vec::new();

    for path in &paths {
        let scenario = match read_scenario(path) {
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
        let provider = args.provider.as_deref().expect("provider required");
        match run_scenario(&scenario, provider, passed + failed) {
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
            other => usage_error(&format!("unknown argument '{other}'")),
        }
    }
    if !args.validate_only && args.provider.is_none() {
        usage_error("--provider <path> is required unless --validate-only is given");
    }
    if let Some(scope) = &args.scope {
        if !["all", "protocol", "semantics"].contains(&scope.as_str()) {
            usage_error("--scope must be all, protocol, or semantics");
        }
    }
    args
}

fn usage_error(message: &str) -> ! {
    eprintln!("lpp-conformance-runner: {message}");
    std::process::exit(2);
}
