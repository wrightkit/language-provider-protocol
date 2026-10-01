//! JSON-RPC 2.0 response envelopes and the handler error channel.
//!
//! LPP errors travel in `error.data.lpp = { kind, details }` with code
//! `-32000`; standard JSON-RPC errors carry only `code` and `message`.

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Errors a method handler can raise: either an LPP-typed error
/// (`kind`/`details` under `error.data.lpp`) or a standard JSON-RPC error.
pub(crate) enum HandlerError {
    Lpp(&'static str, Value, String),
    Std(i64, &'static str),
}

impl HandlerError {
    /// An LPP `refusal` error carrying `refusalCode` in its details.
    pub(crate) fn refusal(code: &'static str, details: Value, message: impl Into<String>) -> Self {
        let mut details = details;
        details["refusalCode"] = json!(code);
        Self::Lpp("refusal", details, message.into())
    }

    /// An LPP-typed error with `kind`/`details`.
    pub(crate) fn lpp(kind: &'static str, details: Value, message: impl Into<String>) -> Self {
        Self::Lpp(kind, details, message.into())
    }

    pub(crate) fn invalid_params() -> Self {
        Self::Std(-32602, "Invalid params")
    }

    pub(crate) fn into_response(self, id: Value) -> Value {
        match self {
            Self::Lpp(kind, details, message) => lpp_error(id, kind, details, message),
            Self::Std(code, message) => std_error(id, code, message),
        }
    }
}

pub(crate) fn parse_params<T: DeserializeOwned>(params: Value) -> Result<T, HandlerError> {
    serde_json::from_value(params).map_err(|_| HandlerError::invalid_params())
}

pub(crate) fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub(crate) fn std_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub(crate) fn lpp_error(
    id: Value,
    kind: &str,
    details: Value,
    message: impl Into<String>,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": message.into(),
            "data": { "lpp": { "kind": kind, "details": details } },
        }
    })
}
