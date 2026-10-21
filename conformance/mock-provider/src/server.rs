use std::collections::HashSet;

use serde_json::{Value, json};

use crate::methods::METHODS;
use crate::rpc::{lpp_error, ok, parse_params, std_error};
use crate::wire::InitParams;

pub(crate) const PROTOCOL_VERSIONS: [&str; 7] = ["1.0", "1.1", "1.2", "1.3", "1.4", "1.5", "1.6"];
pub(crate) const DEFAULT_PROTOCOL_VERSION: &str = PROTOCOL_VERSIONS[0];
pub(crate) const LANGUAGE_ID: &str = "x-demo-lang";

const SERVER_NAME: &str = "lpp-mock-provider";
const LANGUAGE_EXTENSIONS: [&str; 1] = ["xdl"];

struct Capability {
    name: &'static str,
    min_minor: u32,
}

const CAPABILITIES: &[Capability] = &[
    Capability {
        name: "check",
        min_minor: 0,
    },
    Capability {
        name: "compile",
        min_minor: 0,
    },
    Capability {
        name: "reconstruct",
        min_minor: 0,
    },
    Capability {
        name: "symbols",
        min_minor: 0,
    },
    Capability {
        name: "definition",
        min_minor: 0,
    },
    Capability {
        name: "references",
        min_minor: 0,
    },
    Capability {
        name: "rename",
        min_minor: 0,
    },
    Capability {
        name: "editValidation",
        min_minor: 0,
    },
    Capability {
        name: "projectLoading",
        min_minor: 1,
    },
    Capability {
        name: "sourceIdentity",
        min_minor: 3,
    },
    Capability {
        name: "lookup",
        min_minor: 5,
    },
    Capability {
        name: "resolveIds",
        min_minor: 6,
    },
];

fn minor_of(version: &str) -> Option<u32> {
    version.strip_prefix("1.").and_then(|m| m.parse().ok())
}

fn invalid_request() -> Value {
    std_error(Value::Null, -32600, "Invalid Request")
}

fn not_initialized(id: &Value) -> Value {
    lpp_error(
        id.clone(),
        "invalidRequest",
        json!({ "reason": "notInitialized" }),
        "invalid request: session not initialized",
    )
}

pub(crate) fn disable_capability(disabled: &mut HashSet<&'static str>, name: &str) -> bool {
    match CAPABILITIES.iter().find(|cap| cap.name == name) {
        Some(cap) => {
            disabled.insert(cap.name);
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

    pub(crate) fn since(&self, minor: u32) -> bool {
        self.version_minor.is_some_and(|v| v >= minor)
    }

    pub(crate) fn capability(&self, name: &str) -> bool {
        let Some(cap) = CAPABILITIES.iter().find(|cap| cap.name == name) else {
            return false;
        };
        self.since(cap.min_minor) && !self.disabled.contains(name)
    }

    fn capabilities_json(&self) -> Value {
        let minor = self.version_minor.unwrap_or(0);
        CAPABILITIES
            .iter()
            .filter(|cap| minor >= cap.min_minor)
            .map(|cap| (cap.name.to_string(), json!(self.capability(cap.name))))
            .collect::<serde_json::Map<String, Value>>()
            .into()
    }

    pub(crate) fn handle_message(&mut self, line: &str) -> Value {
        let parsed: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return std_error(Value::Null, -32700, "Parse error"),
        };
        let Some(object) = parsed.as_object() else {
            return invalid_request();
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return invalid_request();
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return invalid_request();
        };
        let id = match object.get("id") {
            Some(id @ Value::String(_)) => id.clone(),
            Some(id @ Value::Number(_)) if id.is_i64() || id.is_u64() => id.clone(),
            None | Some(Value::Null) => {
                return lpp_error(
                    Value::Null,
                    "invalidRequest",
                    json!({ "reason": "notificationNotSupported" }),
                    "invalid request: LPP v1 defines no notifications",
                );
            }
            Some(_) => return invalid_request(),
        };
        let params = match object.get("params") {
            Some(params @ Value::Object(_)) => params.clone(),
            _ => return std_error(id.clone(), -32602, "Invalid params"),
        };
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
