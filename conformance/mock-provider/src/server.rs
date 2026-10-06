//! Session state and JSON-RPC method dispatch.
//!
//! `METHODS` (defined next to the handlers in `methods.rs`) is the single
//! table routing a method name to its capability gate and handler.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::methods::METHODS;
use crate::rpc::{lpp_error, ok, parse_params, std_error};
use crate::wire::InitParams;

/// Protocol versions accepted by `--protocol-version`.
pub(crate) const PROTOCOL_VERSIONS: [&str; 6] = ["1.0", "1.1", "1.2", "1.3", "1.4", "1.5"];
pub(crate) const DEFAULT_PROTOCOL_VERSION: &str = PROTOCOL_VERSIONS[0];
pub(crate) const LANGUAGE_ID: &str = "x-demo-lang";

const SERVER_NAME: &str = "lpp-mock-provider";
const LANGUAGE_EXTENSIONS: [&str; 1] = ["xdl"];

/// Capability names as they appear on the wire, each with the minimum
/// protocol minor version that advertises it.
const CAPABILITIES: &[(&str, u32)] = &[
    ("check", 0),
    ("compile", 0),
    ("reconstruct", 0),
    ("symbols", 0),
    ("definition", 0),
    ("references", 0),
    ("rename", 0),
    ("editValidation", 0),
    ("projectLoading", 1),
    ("sourceIdentity", 3),
    ("lookup", 5),
];

fn minor_of(version: &str) -> Option<u32> {
    version.strip_prefix("1.").and_then(|m| m.parse().ok())
}

fn invalid_request() -> Value {
    std_error(Value::Null, -32600, "Invalid Request")
}

/// `invalidRequest`/`notInitialized` for any method called pre-`initialize`.
fn not_initialized(id: &Value) -> Value {
    lpp_error(
        id.clone(),
        "invalidRequest",
        json!({ "reason": "notInitialized" }),
        "invalid request: session not initialized",
    )
}

/// Mark `name` disabled for `--without`. Returns false for unknown names.
pub(crate) fn disable_capability(disabled: &mut HashSet<&'static str>, name: &str) -> bool {
    match CAPABILITIES.iter().find(|(n, _)| *n == name) {
        Some(&(n, _)) => {
            disabled.insert(n);
            true
        }
        None => false,
    }
}

pub(crate) struct Server {
    initialized: bool,
    pub(crate) exiting: bool,
    version_minor: Option<u32>,
    supported_version: String,
    disabled: HashSet<&'static str>,
}

impl Server {
    pub(crate) fn new(supported_version: String, disabled: HashSet<&'static str>) -> Self {
        Self {
            initialized: false,
            exiting: false,
            version_minor: None,
            supported_version,
            disabled,
        }
    }

    /// True when the negotiated protocol version is 1.`minor` or later.
    pub(crate) fn since(&self, minor: u32) -> bool {
        self.version_minor.is_some_and(|v| v >= minor)
    }

    /// True when `capability` exists at the negotiated protocol version and
    /// is not disabled via `--without`. Unknown names are never available.
    pub(crate) fn capability(&self, name: &str) -> bool {
        let Some(&(_, min)) = CAPABILITIES.iter().find(|(n, _)| *n == name) else {
            return false;
        };
        self.since(min) && !self.disabled.contains(name)
    }

    fn capabilities_json(&self) -> Value {
        let minor = self.version_minor.unwrap_or(0);
        CAPABILITIES
            .iter()
            .filter(|(_, min)| minor >= *min)
            .map(|(name, _)| ((*name).to_string(), json!(self.capability(name))))
            .collect::<serde_json::Map<String, Value>>()
            .into()
    }

    /// Handle one newline-delimited message and return the response envelope.
    pub(crate) fn handle_message(&mut self, line: &str) -> Value {
        let parsed: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return std_error(Value::Null, -32700, "Parse error"),
        };
        let Some(object) = parsed.as_object() else {
            // Batches are arrays; both fail as Invalid Request.
            return invalid_request();
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return invalid_request();
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return invalid_request();
        };
        let id = match object.get("id") {
            Some(id @ (Value::Number(_) | Value::String(_))) => id.clone(),
            // No notifications in LPP v1: a message without an id (or with a
            // null id) is a protocol violation.
            _ => {
                return lpp_error(
                    Value::Null,
                    "invalidRequest",
                    json!({ "reason": "notificationNotSupported" }),
                    "invalid request: LPP v1 defines no notifications",
                );
            }
        };
        let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
        match method {
            "lpp/initialize" => self.initialize(&id, params),
            "lpp/shutdown" => self.shutdown(&id),
            _ => self.dispatch(&id, method, params),
        }
    }

    fn initialize(&mut self, id: &Value, params: Value) -> Value {
        if self.initialized {
            return lpp_error(
                id.clone(),
                "invalidRequest",
                json!({ "reason": "alreadyInitialized" }),
                "invalid request: already initialized",
            );
        }
        let params: InitParams = match parse_params(params) {
            Ok(params) => params,
            Err(_) => return std_error(id.clone(), -32602, "Invalid params"),
        };
        if params.protocol_version != self.supported_version {
            return lpp_error(
                id.clone(),
                "protocolVersionMismatch",
                json!({
                    "supportedProtocolVersions": [self.supported_version]
                }),
                format!("unsupported protocol version {}", params.protocol_version),
            );
        }
        self.version_minor = minor_of(&params.protocol_version);
        self.initialized = true;
        ok(
            id.clone(),
            json!({
                "protocolVersion": params.protocol_version,
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "languages": [
                    { "id": LANGUAGE_ID, "extensions": LANGUAGE_EXTENSIONS },
                ],
                "capabilities": self.capabilities_json(),
            }),
        )
    }

    fn shutdown(&mut self, id: &Value) -> Value {
        if !self.initialized {
            return not_initialized(id);
        }
        self.exiting = true;
        ok(id.clone(), Value::Null)
    }

    fn dispatch(&mut self, id: &Value, method: &str, params: Value) -> Value {
        if !self.initialized {
            return not_initialized(id);
        }
        let Some(entry) = METHODS.iter().find(|m| m.name == method) else {
            return std_error(id.clone(), -32601, "Method not found");
        };
        if !self.capability(entry.capability) {
            return lpp_error(
                id.clone(),
                "capabilityUnavailable",
                json!({ "capability": entry.capability, "method": method }),
                format!("capability '{}' is not available", entry.capability),
            );
        }
        match (entry.handler)(self, params) {
            Ok(result) => ok(id.clone(), result),
            Err(error) => error.into_response(id.clone()),
        }
    }
}
