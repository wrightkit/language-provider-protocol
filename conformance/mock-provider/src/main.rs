mod methods;
mod project;
mod puzzle;
mod rpc;
mod server;
mod wire;

use std::collections::HashSet;
use std::io::{self, BufRead, BufWriter, Write};

use serde_json::Value;

use server::{DEFAULT_PROTOCOL_VERSION, PROTOCOL_VERSIONS, Server};

fn main() {
    let (supported_version, disabled) = parse_args();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut server = Server::new(supported_version, disabled);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = match reader.read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(_) => break,
        };
        if read == 0 {
            break;
        }
        let response = match std::str::from_utf8(&buf) {
            Ok(line) => {
                let message = line.trim_end_matches(['\r', '\n']);
                if message.is_empty() {
                    continue;
                }
                server.handle_message(message)
            }
            // A line that is not valid UTF-8 cannot be valid JSON: -32700 and
            // the session continues.
            Err(_) => rpc::std_error(Value::Null, -32700, "Parse error"),
        };
        if write_response(&mut out, &response).is_err() {
            break;
        }
        if server.exiting {
            break;
        }
    }
}

fn write_response(out: &mut impl Write, response: &Value) -> io::Result<()> {
    let serialized = serde_json::to_string(response).expect("response serializes");
    out.write_all(serialized.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

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
