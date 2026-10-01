//! `lpp-mock-provider`: the reference LPP provider for `x-demo-lang`.
//!
//! Layout: `server` owns session state and dispatch (the method table lives
//! next to the handlers in `methods`), `project` resolves `entry` targets
//! into document sets, `wire` holds the request param shapes, `rpc` builds
//! response envelopes, and `puzzle` is the x-demo-lang implementation.

mod methods;
mod project;
mod puzzle;
mod rpc;
mod server;
mod wire;

use std::collections::HashSet;
use std::io::{self, BufRead, BufWriter, Write};

use server::{DEFAULT_PROTOCOL_VERSION, PROTOCOL_VERSIONS, Server};

fn main() {
    let (supported_version, disabled) = parse_args();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut server = Server::new(supported_version, disabled);
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .unwrap_or_else(|e| panic!("failed to read stdin: {e}"));
        if read == 0 {
            break;
        }
        let message = line.trim_end_matches(['\r', '\n']);
        if message.is_empty() {
            continue;
        }
        let response = server.handle_message(message);
        let serialized = serde_json::to_string(&response).expect("response serializes");
        writeln!(out, "{serialized}").expect("write stdout");
        out.flush().expect("flush stdout");
        if server.exiting {
            break;
        }
    }
}

/// `--protocol-version <v>` pins the supported version; `--without a,b,c`
/// disables capabilities (used by the capability-negotiation fixtures).
fn parse_args() -> (String, HashSet<&'static str>) {
    let mut version = DEFAULT_PROTOCOL_VERSION.to_string();
    let mut disabled = HashSet::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--protocol-version" => {
                let value = arg_value(&mut args, "--protocol-version");
                if !PROTOCOL_VERSIONS.contains(&value.as_str()) {
                    usage_error(&format!("unsupported protocol version '{value}'"));
                }
                version = value;
            }
            "--without" => {
                for name in arg_value(&mut args, "--without").split(',') {
                    if !server::disable_capability(&mut disabled, name) {
                        usage_error(&format!("unknown capability '{name}'"));
                    }
                }
            }
            other => usage_error(&format!("unknown argument '{other}'")),
        }
    }
    (version, disabled)
}

fn arg_value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    match args.next() {
        Some(value) => value,
        None => usage_error(&format!("{flag} requires a value")),
    }
}

fn usage_error(message: &str) -> ! {
    eprintln!("lpp-mock-provider: {message}");
    std::process::exit(2);
}
