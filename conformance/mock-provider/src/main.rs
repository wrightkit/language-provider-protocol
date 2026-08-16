//! LPP v1 conformance mock provider for `x-demo-lang`.
//!
//! A small stdio binary implementing the Language Provider Protocol v1
//! (spec/lpp-v1.md). It serves the deliberately non-OPY/DEL language
//! `x-demo-lang`: an equation-puzzle DSL. Compilation simulates the puzzle's
//! solution and emits a puzzle evaluation sheet in the provider's own
//! artifact format.
//!
//! Usage: `lpp-mock-provider [--without <capability>,...]`

mod puzzle;

use std::collections::HashMap;
use std::io::{self, BufRead, BufWriter, Write};

use serde::Deserialize;
use serde_json::{Value, json};

use puzzle::{
    ARTIFACT_FORMAT, KIND_OP, KIND_PUZZLE, ParseOutput, Range, SourceText, compile_artifact,
    is_valid_identifier, parse_document, reconstruct_source, symbol_at, validate_edits,
};

const PROTOCOL_VERSION: &str = "1.0";
const SERVER_NAME: &str = "lpp-mock-provider";
const LANGUAGE_ID: &str = "x-demo-lang";
const LANGUAGE_EXTENSIONS: [&str; 1] = ["xdl"];

#[derive(Debug, Clone, Copy)]
struct Capabilities {
    check: bool,
    compile: bool,
    reconstruct: bool,
    symbols: bool,
    definition: bool,
    references: bool,
    rename: bool,
    edit_validation: bool,
}

impl Capabilities {
    fn all() -> Self {
        Self {
            check: true,
            compile: true,
            reconstruct: true,
            symbols: true,
            definition: true,
            references: true,
            rename: true,
            edit_validation: true,
        }
    }

    fn without(&mut self, name: &str) -> bool {
        let field = match name {
            "check" => &mut self.check,
            "compile" => &mut self.compile,
            "reconstruct" => &mut self.reconstruct,
            "symbols" => &mut self.symbols,
            "definition" => &mut self.definition,
            "references" => &mut self.references,
            "rename" => &mut self.rename,
            "editValidation" => &mut self.edit_validation,
            _ => return false,
        };
        *field = false;
        true
    }

    fn enabled(&self, name: &str) -> bool {
        match name {
            "check" => self.check,
            "compile" => self.compile,
            "reconstruct" => self.reconstruct,
            "symbols" => self.symbols,
            "definition" => self.definition,
            "references" => self.references,
            "rename" => self.rename,
            "editValidation" => self.edit_validation,
            _ => false,
        }
    }

    fn to_json(self) -> Value {
        json!({
            "check": self.check,
            "compile": self.compile,
            "reconstruct": self.reconstruct,
            "symbols": self.symbols,
            "definition": self.definition,
            "references": self.references,
            "rename": self.rename,
            "editValidation": self.edit_validation,
        })
    }
}

/// The capability id governing a method, or `None` for unknown methods.
fn capability_of(method: &str) -> Option<&'static str> {
    Some(match method {
        "lpp/check" => "check",
        "lpp/compile" => "compile",
        "lpp/reconstruct" => "reconstruct",
        "lpp/symbols" => "symbols",
        "lpp/definition" => "definition",
        "lpp/references" => "references",
        "lpp/rename" => "rename",
        "lpp/validateEdits" => "editValidation",
        _ => return None,
    })
}

/// A handler failure. `Lpp` maps to the LPP error envelope (code `-32000`
/// with `data.lpp`); `Std` maps to a standard JSON-RPC error code.
enum HandlerError {
    Lpp(&'static str, Value, String),
    Std(i64, &'static str),
}

impl HandlerError {
    fn refusal(code: &str, details: Value, message: &str) -> Self {
        let mut details = details;
        details["refusalCode"] = json!(code);
        HandlerError::Lpp("refusal", details, message.to_string())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitParams {
    protocol_version: String,
    #[allow(dead_code)]
    client_info: Option<ClientInfo>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    uri: String,
    language_id: String,
    version: i64,
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    format: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocsParams {
    documents: HashMap<String, Document>,
    #[allow(dead_code)]
    project_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReconstructParams {
    artifact: Artifact,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PositionParams {
    document: Document,
    position: puzzle::Position,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferencesParams {
    document: Document,
    position: puzzle::Position,
    include_declaration: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameParams {
    documents: HashMap<String, Document>,
    position_document_uri: String,
    position: puzzle::Position,
    new_name: String,
    #[allow(dead_code)]
    project_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateEditsParams {
    document: Document,
    edits: Vec<WireTextEdit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTextEdit {
    range: Range,
    new_text: String,
}

struct Server {
    initialized: bool,
    exiting: bool,
    caps: Capabilities,
}

fn main() {
    let caps = parse_args();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut server = Server {
        initialized: false,
        exiting: false,
        caps,
    };
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .unwrap_or_else(|e| panic!("failed to read stdin: {e}"));
        if read == 0 {
            // EOF: exit cleanly with status 0.
            break;
        }
        let message = line.trim_end_matches(['\r', '\n']);
        if message.is_empty() {
            continue;
        }
        let response = server.handle_message(message);
        if let Some(response) = response {
            let serialized = serde_json::to_string(&response).expect("response serializes");
            writeln!(out, "{serialized}").expect("write stdout");
            out.flush().expect("flush stdout");
        }
        if server.exiting {
            break;
        }
    }
}

fn parse_args() -> Capabilities {
    let mut caps = Capabilities::all();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--without" => {
                i += 1;
                let Some(list) = args.get(i) else {
                    eprintln!(
                        "lpp-mock-provider: --without requires a comma-separated capability list"
                    );
                    std::process::exit(2);
                };
                for name in list.split(',') {
                    if !caps.without(name) {
                        eprintln!("lpp-mock-provider: unknown capability '{name}'");
                        std::process::exit(2);
                    }
                }
            }
            other => {
                eprintln!("lpp-mock-provider: unknown argument '{other}'");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    caps
}

impl Server {
    fn handle_message(&mut self, line: &str) -> Option<Value> {
        let parsed: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return Some(std_error(Value::Null, -32700, "Parse error")),
        };
        if parsed.is_array() {
            return Some(std_error(Value::Null, -32600, "Invalid Request"));
        }
        let Some(object) = parsed.as_object() else {
            return Some(std_error(Value::Null, -32600, "Invalid Request"));
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(std_error(Value::Null, -32600, "Invalid Request"));
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Some(std_error(Value::Null, -32600, "Invalid Request"));
        };
        let id = match object.get("id") {
            Some(id @ (Value::Number(_) | Value::String(_))) => id.clone(),
            _ => {
                // No notifications in LPP v1: a message without an id (or
                // with a null id) is a protocol violation.
                return Some(lpp_error(
                    Value::Null,
                    "invalidRequest",
                    json!({ "reason": "notificationNotSupported" }),
                    "invalid request: LPP v1 defines no notifications",
                ));
            }
        };
        let params = object.get("params").cloned().unwrap_or_else(|| json!({}));

        let response = match method {
            "lpp/initialize" => self.initialize(&id, params),
            "lpp/shutdown" => self.shutdown(&id),
            _ => self.dispatch(&id, method, params),
        };
        Some(response)
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
        let params: InitParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(_) => return std_error(id.clone(), -32602, "Invalid params"),
        };
        if params.protocol_version != PROTOCOL_VERSION {
            return lpp_error(
                id.clone(),
                "protocolVersionMismatch",
                json!({ "supportedProtocolVersions": [PROTOCOL_VERSION] }),
                format!("unsupported protocol version {}", params.protocol_version),
            );
        }
        self.initialized = true;
        ok(
            id.clone(),
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "languages": [
                    { "id": LANGUAGE_ID, "extensions": LANGUAGE_EXTENSIONS },
                ],
                "capabilities": self.caps.to_json(),
            }),
        )
    }

    fn shutdown(&mut self, id: &Value) -> Value {
        if !self.initialized {
            return lpp_error(
                id.clone(),
                "invalidRequest",
                json!({ "reason": "notInitialized" }),
                "invalid request: session not initialized",
            );
        }
        self.exiting = true;
        ok(id.clone(), Value::Null)
    }

    fn dispatch(&mut self, id: &Value, method: &str, params: Value) -> Value {
        if !self.initialized {
            return lpp_error(
                id.clone(),
                "invalidRequest",
                json!({ "reason": "notInitialized" }),
                "invalid request: session not initialized",
            );
        }
        let Some(capability) = capability_of(method) else {
            return std_error(id.clone(), -32601, "Method not found");
        };
        if !self.caps.enabled(capability) {
            return lpp_error(
                id.clone(),
                "capabilityUnavailable",
                json!({ "capability": capability, "method": method }),
                format!("capability '{capability}' is not available"),
            );
        }
        let result = match method {
            "lpp/check" => self.check(params),
            "lpp/compile" => self.compile(params),
            "lpp/reconstruct" => self.reconstruct(params),
            "lpp/symbols" => self.symbols(params),
            "lpp/definition" => self.definition(params),
            "lpp/references" => self.references(params),
            "lpp/rename" => self.rename(params),
            "lpp/validateEdits" => self.validate_edits(params),
            _ => return std_error(id.clone(), -32601, "Method not found"),
        };
        match result {
            Ok(value) => ok(id.clone(), value),
            Err(error) => match error {
                HandlerError::Lpp(kind, details, message) => {
                    lpp_error(id.clone(), kind, details, message)
                }
                HandlerError::Std(code, message) => std_error(id.clone(), code, message),
            },
        }
    }

    fn check(&self, params: Value) -> Result<Value, HandlerError> {
        let params: DocsParams = parse_params(params)?;
        let mut documents = Vec::new();
        for uri in &sorted_keys(&params.documents) {
            let doc = &params.documents[uri];
            check_document(doc)?;
            let parsed = parse_document(&doc.text);
            documents.push(json!({
                "uri": doc.uri,
                "version": doc.version,
                "diagnostics": parsed.diagnostics,
            }));
        }
        Ok(json!({ "documents": documents }))
    }

    fn compile(&self, params: Value) -> Result<Value, HandlerError> {
        let params: DocsParams = parse_params(params)?;
        if params.documents.len() != 1 {
            return Err(HandlerError::refusal(
                "compile.requiresSingleDocument",
                json!({}),
                "compile requires exactly one document",
            ));
        }
        let doc = params.documents.values().next().expect("len == 1");
        check_document(doc)?;
        let parsed = parse_document(&doc.text);
        let has_errors = parsed.diagnostics.iter().any(|d| d.severity == "error");
        let artifact = if has_errors {
            Value::Null
        } else {
            let puzzle = parsed.puzzle.as_ref().expect("no errors implies a puzzle");
            let value = puzzle.simulate().expect("no errors implies resolvable ops");
            let content = serde_json::to_string(&compile_artifact(puzzle, value))
                .expect("artifact serializes");
            json!({ "format": ARTIFACT_FORMAT, "content": content })
        };
        Ok(json!({
            "diagnostics": [{
                "uri": doc.uri,
                "version": doc.version,
                "diagnostics": parsed.diagnostics,
            }],
            "artifact": artifact,
        }))
    }

    fn reconstruct(&self, params: Value) -> Result<Value, HandlerError> {
        let params: ReconstructParams = parse_params(params)?;
        if params.artifact.format != ARTIFACT_FORMAT {
            return Err(HandlerError::refusal(
                "reconstruct.artifactFormatUnsupported",
                json!({ "format": params.artifact.format }),
                "artifact format not supported",
            ));
        }
        match reconstruct_source(&params.artifact.content) {
            Some(source) => Ok(json!({ "source": source })),
            None => Err(HandlerError::Lpp(
                "invalidArtifact",
                json!({ "reason": "malformedArtifactContent" }),
                "artifact content is not a valid puzzle evaluation sheet".to_string(),
            )),
        }
    }

    fn symbols(&self, params: Value) -> Result<Value, HandlerError> {
        let params: DocsParams = parse_params(params)?;
        let mut documents = Vec::new();
        for uri in &sorted_keys(&params.documents) {
            let doc = &params.documents[uri];
            check_document(doc)?;
            let parsed = parse_document(&doc.text);
            let symbols = parsed
                .puzzle
                .as_ref()
                .map(|p| {
                    let mut symbols = vec![json!({
                        "name": p.name,
                        "kind": KIND_PUZZLE,
                        "range": p.name_range,
                    })];
                    for op in &p.ops {
                        symbols.push(json!({
                            "name": op.name,
                            "kind": KIND_OP,
                            "range": op.name_range,
                        }));
                    }
                    symbols
                })
                .unwrap_or_default();
            documents.push(json!({
                "uri": doc.uri,
                "version": doc.version,
                "symbols": symbols,
            }));
        }
        Ok(json!({ "documents": documents }))
    }

    fn definition(&self, params: Value) -> Result<Value, HandlerError> {
        let params: PositionParams = parse_params(params)?;
        check_document(&params.document)?;
        let src = SourceText::new(&params.document.text);
        let Some(byte) = src.byte_of(params.position) else {
            return Err(HandlerError::Lpp(
                "invalidPosition",
                json!({ "uri": params.document.uri, "position": params.position }),
                "position outside document".to_string(),
            ));
        };
        let Some(parsed) = parse_ok(&params.document) else {
            return Err(HandlerError::refusal(
                "definition.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let Some(puzzle) = &parsed.puzzle else {
            return Err(HandlerError::refusal(
                "definition.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let Some(symbol) = symbol_at(&src, puzzle, byte) else {
            return Err(HandlerError::refusal(
                "definition.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let range = match symbol {
            puzzle::Symbol::Puzzle { range, .. } => range,
            puzzle::Symbol::Op { name, .. } => match puzzle.op(&name) {
                Some(op) => op.name_range,
                None => {
                    return Err(HandlerError::refusal(
                        "definition.noSymbolAtPosition",
                        json!({ "uri": params.document.uri }),
                        "no symbol at position",
                    ));
                }
            },
        };
        Ok(json!({
            "locations": [{ "uri": params.document.uri, "range": range }]
        }))
    }

    fn references(&self, params: Value) -> Result<Value, HandlerError> {
        let params: ReferencesParams = parse_params(params)?;
        check_document(&params.document)?;
        let src = SourceText::new(&params.document.text);
        let Some(byte) = src.byte_of(params.position) else {
            return Err(HandlerError::Lpp(
                "invalidPosition",
                json!({ "uri": params.document.uri, "position": params.position }),
                "position outside document".to_string(),
            ));
        };
        let Some(parsed) = parse_ok(&params.document) else {
            return Err(HandlerError::refusal(
                "references.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let Some(puzzle) = &parsed.puzzle else {
            return Err(HandlerError::refusal(
                "references.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let Some(symbol) = symbol_at(&src, puzzle, byte) else {
            return Err(HandlerError::refusal(
                "references.noSymbolAtPosition",
                json!({ "uri": params.document.uri }),
                "no symbol at position",
            ));
        };
        let name = symbol.name();
        let mut locations = Vec::new();
        if params.include_declaration {
            if let Some(op) = puzzle.op(name) {
                locations.push(json!({ "uri": params.document.uri, "range": op.name_range }));
            }
        }
        for entry in &puzzle.solution {
            if entry.name == name {
                locations.push(json!({ "uri": params.document.uri, "range": entry.range }));
            }
        }
        Ok(json!({ "locations": locations }))
    }

    fn rename(&self, params: Value) -> Result<Value, HandlerError> {
        let params: RenameParams = parse_params(params)?;
        for uri in &sorted_keys(&params.documents) {
            check_document(&params.documents[uri])?;
        }
        if !is_valid_identifier(&params.new_name) {
            return Err(HandlerError::refusal(
                "rename.invalidName",
                json!({ "newName": params.new_name }),
                "new name is not a valid identifier",
            ));
        }
        let Some(position_doc) = params.documents.get(&params.position_document_uri) else {
            return Err(HandlerError::Lpp(
                "invalidDocument",
                json!({
                    "uri": params.position_document_uri,
                    "reason": "positionDocumentUriNotInSet",
                }),
                "position document is not in the document set".to_string(),
            ));
        };
        let position_src = SourceText::new(&position_doc.text);
        let Some(byte) = position_src.byte_of(params.position) else {
            return Err(HandlerError::Lpp(
                "invalidPosition",
                json!({ "uri": params.position_document_uri, "position": params.position }),
                "position outside document".to_string(),
            ));
        };
        let Some(parsed) = parse_ok(position_doc) else {
            return Err(HandlerError::refusal(
                "rename.noSymbolAtPosition",
                json!({ "uri": params.position_document_uri }),
                "no symbol at position",
            ));
        };
        let Some(puzzle) = &parsed.puzzle else {
            return Err(HandlerError::refusal(
                "rename.noSymbolAtPosition",
                json!({ "uri": params.position_document_uri }),
                "no symbol at position",
            ));
        };
        let Some(symbol) = symbol_at(&position_src, puzzle, byte) else {
            return Err(HandlerError::refusal(
                "rename.noSymbolAtPosition",
                json!({ "uri": params.position_document_uri }),
                "no symbol at position",
            ));
        };

        let target = match symbol {
            puzzle::Symbol::Puzzle { name, .. } => RenameTarget::Puzzle(name),
            puzzle::Symbol::Op { name, .. } => {
                let collides = puzzle.op(&name).is_some()
                    && name != params.new_name
                    && puzzle.op(&params.new_name).is_some();
                if collides {
                    return Err(HandlerError::refusal(
                        "rename.nameCollision",
                        json!({ "newName": params.new_name }),
                        "new name collides with an existing symbol",
                    ));
                }
                RenameTarget::Op(name)
            }
        };

        let mut edits = Vec::new();
        for uri in &sorted_keys(&params.documents) {
            let doc = &params.documents[uri];
            let parsed = parse_document(&doc.text);
            let Some(puzzle) = parsed.puzzle else {
                continue;
            };
            let mut text_edits = Vec::new();
            match &target {
                RenameTarget::Puzzle(name) => {
                    if uri == &params.position_document_uri && puzzle.name == *name {
                        text_edits.push(json!({
                            "range": puzzle.name_range,
                            "newText": params.new_name,
                        }));
                    }
                }
                RenameTarget::Op(name) => {
                    if let Some(op) = puzzle.op(name) {
                        text_edits.push(json!({
                            "range": op.name_range,
                            "newText": params.new_name,
                        }));
                    }
                    for entry in &puzzle.solution {
                        if entry.name == *name {
                            text_edits.push(json!({
                                "range": entry.range,
                                "newText": params.new_name,
                            }));
                        }
                    }
                }
            }
            if !text_edits.is_empty() {
                edits.push(json!({
                    "documentUri": doc.uri,
                    "version": doc.version,
                    "textEdits": text_edits,
                }));
            }
        }
        if edits.is_empty() {
            return Err(HandlerError::refusal(
                "rename.noSymbolAtPosition",
                json!({ "uri": params.position_document_uri }),
                "no symbol at position",
            ));
        }
        Ok(json!({ "edits": edits }))
    }

    fn validate_edits(&self, params: Value) -> Result<Value, HandlerError> {
        let params: ValidateEditsParams = parse_params(params)?;
        check_document(&params.document)?;
        let edits: Vec<(Range, String)> = params
            .edits
            .iter()
            .map(|e| (e.range, e.new_text.clone()))
            .collect();
        match validate_edits(&params.document.text, &edits) {
            puzzle::EditValidation::Valid => {
                Ok(json!({ "valid": true, "version": params.document.version }))
            }
            puzzle::EditValidation::Invalid {
                reason,
                failing_edit_index,
            } => {
                let mut result = json!({
                    "valid": false,
                    "version": params.document.version,
                    "reason": reason,
                });
                if let Some(index) = failing_edit_index {
                    result["failingEditIndex"] = json!(index);
                }
                Ok(result)
            }
        }
    }
}

#[derive(Debug)]
enum RenameTarget {
    Puzzle(String),
    Op(String),
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, HandlerError> {
    serde_json::from_value(params).map_err(|_| HandlerError::Std(-32602, "Invalid params"))
}

fn sorted_keys(map: &HashMap<String, Document>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys
}

/// Validate the document-level invariants (language id, version).
fn check_document(doc: &Document) -> Result<(), HandlerError> {
    if doc.language_id != LANGUAGE_ID {
        return Err(HandlerError::Lpp(
            "invalidLanguage",
            json!({ "languageId": doc.language_id }),
            format!(
                "language '{}' is not served by this provider",
                doc.language_id
            ),
        ));
    }
    if doc.version < 0 {
        return Err(HandlerError::Lpp(
            "invalidDocument",
            json!({ "uri": doc.uri, "reason": "invalidVersion" }),
            "document version must be a non-negative integer".to_string(),
        ));
    }
    Ok(())
}

/// Parse a document and require a parseable puzzle (no error diagnostics).
fn parse_ok(doc: &Document) -> Option<ParseOutput> {
    let parsed = parse_document(&doc.text);
    let has_errors = parsed.diagnostics.iter().any(|d| d.severity == "error");
    if has_errors || parsed.puzzle.is_none() {
        return None;
    }
    Some(parsed)
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn std_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn lpp_error(id: Value, kind: &str, details: Value, message: impl Into<String>) -> Value {
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
