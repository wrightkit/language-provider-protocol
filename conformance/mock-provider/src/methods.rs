//! `lpp/*` method handlers and the `METHODS` dispatch table.
//!
//! Handlers share three pieces of plumbing: `documents_for_request` resolves
//! the document set (inline `documents` or a loaded `entry` project),
//! `parsed_set` checks and parses it in URI order, and `symbol_at_position`
//! resolves the document/position/symbol preamble that definition,
//! references, and rename share.

use std::collections::HashMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::project::documents_for_request;
use crate::puzzle::{
    self, ARTIFACT_FORMAT, COMPILE_ARTIFACT_FORMATS, LookupScope, ParseOutput, Position, Puzzle,
    Range, SUMMARY_ARTIFACT_FORMAT, SourceText, Symbol, compile_artifact, is_valid_identifier,
    parse_document, reconstruct_source, summary_artifact,
};
use crate::rpc::{HandlerError, parse_params};
use crate::server::{LANGUAGE_ID, Server};
use crate::wire::{
    DocsParams, Document, LookupParams, PositionParams, ReconstructParams, ReferencesParams,
    RenameParams, ValidateEditsParams,
};

/// A dispatchable `lpp/*` method: wire name, gating capability, handler.
pub(crate) struct Method {
    pub name: &'static str,
    pub capability: &'static str,
    pub handler: fn(&Server, Value) -> Result<Value, HandlerError>,
}

/// Every `lpp/*` request method. Dispatch, capability gating, and the
/// `Method not found` fallback all route through this table.
pub(crate) const METHODS: &[Method] = &[
    Method {
        name: "lpp/check",
        capability: "check",
        handler: Server::check,
    },
    Method {
        name: "lpp/compile",
        capability: "compile",
        handler: Server::compile,
    },
    Method {
        name: "lpp/reconstruct",
        capability: "reconstruct",
        handler: Server::reconstruct,
    },
    Method {
        name: "lpp/symbols",
        capability: "symbols",
        handler: Server::symbols,
    },
    Method {
        name: "lpp/definition",
        capability: "definition",
        handler: Server::definition,
    },
    Method {
        name: "lpp/references",
        capability: "references",
        handler: Server::references,
    },
    Method {
        name: "lpp/rename",
        capability: "rename",
        handler: Server::rename,
    },
    Method {
        name: "lpp/validateEdits",
        capability: "editValidation",
        handler: Server::validate_edits,
    },
    Method {
        name: "lpp/lookup",
        capability: "lookup",
        handler: Server::lookup,
    },
];

impl Server {
    fn check(&self, params: Value) -> Result<Value, HandlerError> {
        let (documents, _) = documents_for_request(self, parse_params(params)?, "lpp/check")?;
        let documents = parsed_set(&documents)?
            .into_iter()
            .map(|(doc, parsed)| document_view(doc, "diagnostics", json!(parsed.diagnostics)))
            .collect::<Vec<_>>();
        Ok(json!({ "documents": documents }))
    }

    fn compile(&self, params: Value) -> Result<Value, HandlerError> {
        let accepted = accepted_artifact_formats(self, &params)?;
        let (documents, entry_uri) =
            documents_for_request(self, parse_params(params)?, "lpp/compile")?;
        if entry_uri.is_none() && documents.len() != 1 {
            return Err(HandlerError::refusal(
                "compile.requiresSingleDocument",
                json!({}),
                "compile requires exactly one document",
            ));
        }
        let entry = match &entry_uri {
            Some(uri) => &documents[uri],
            None => documents.values().next().expect("len == 1"),
        };
        check_document(entry)?;
        let parsed = parsed_set(&documents)?;
        let has_errors = parsed.iter().any(|(_, out)| out.has_errors());
        let diagnostics = parsed
            .iter()
            .map(|(doc, out)| document_view(doc, "diagnostics", json!(out.diagnostics)))
            .collect::<Vec<_>>();
        let artifact = if has_errors {
            Value::Null
        } else {
            let format = select_artifact_format(accepted)?;
            let puzzle = parse_document(&entry.text)
                .puzzle
                .expect("no errors implies a puzzle");
            let value = puzzle.simulate().expect("no errors implies resolvable ops");
            let artifact = if format == SUMMARY_ARTIFACT_FORMAT {
                summary_artifact(&puzzle, value)
            } else {
                compile_artifact(&puzzle, value)
            };
            let content = serde_json::to_string(&artifact).expect("artifact serializes");
            json!({ "format": format, "content": content })
        };
        let mut result = json!({ "diagnostics": diagnostics, "artifact": artifact });
        if self.capability("sourceIdentity") {
            if let Some(uri) = &entry_uri {
                result["sourceIdentity"] = json!(sha256_hex(&documents[uri].text));
            }
        }
        Ok(result)
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
            None => Err(HandlerError::lpp(
                "invalidArtifact",
                json!({ "reason": "malformedArtifactContent" }),
                "artifact content is not a valid puzzle evaluation sheet",
            )),
        }
    }

    fn symbols(&self, params: Value) -> Result<Value, HandlerError> {
        let params: DocsParams = parse_params(params)?;
        if params.entry.is_some() {
            return Err(HandlerError::invalid_params());
        }
        let Some(documents) = params.documents else {
            return Err(HandlerError::invalid_params());
        };
        let documents = parsed_set(&documents)?
            .into_iter()
            .map(|(doc, parsed)| {
                let symbols = parsed
                    .puzzle
                    .as_ref()
                    .map(Puzzle::document_symbols)
                    .unwrap_or_default();
                document_view(doc, "symbols", json!(symbols))
            })
            .collect::<Vec<_>>();
        Ok(json!({ "documents": documents }))
    }

    fn definition(&self, params: Value) -> Result<Value, HandlerError> {
        let params: PositionParams = parse_params(params)?;
        let (puzzle, symbol) = symbol_at_position(
            &params.document,
            params.position,
            "definition.noSymbolAtPosition",
            &params.document.uri,
        )?;
        let range = puzzle
            .declaration_range(&symbol)
            .expect("resolved puzzles reference declared ops");
        Ok(json!({
            "locations": [{ "uri": params.document.uri, "range": range }]
        }))
    }

    fn references(&self, params: Value) -> Result<Value, HandlerError> {
        let params: ReferencesParams = parse_params(params)?;
        let (puzzle, symbol) = symbol_at_position(
            &params.document,
            params.position,
            "references.noSymbolAtPosition",
            &params.document.uri,
        )?;
        let locations: Vec<Value> = puzzle
            .occurrences(symbol.name(), params.include_declaration)
            .into_iter()
            .map(|range| location_json(&params.document.uri, range))
            .collect();
        Ok(json!({ "locations": locations }))
    }

    fn rename(&self, params: Value) -> Result<Value, HandlerError> {
        let params: RenameParams = parse_params(params)?;
        for doc in sorted_docs(&params.documents) {
            check_document(doc)?;
        }
        if !is_valid_identifier(&params.new_name) {
            return Err(HandlerError::refusal(
                "rename.invalidName",
                json!({ "newName": params.new_name }),
                "new name is not a valid identifier",
            ));
        }
        let Some(position_doc) = params.documents.get(&params.position_document_uri) else {
            return Err(HandlerError::lpp(
                "invalidDocument",
                json!({
                    "uri": params.position_document_uri,
                    "reason": "positionDocumentUriNotInSet",
                }),
                "position document is not in the document set",
            ));
        };
        let (puzzle, symbol) = symbol_at_position(
            position_doc,
            params.position,
            "rename.noSymbolAtPosition",
            &params.position_document_uri,
        )?;

        if puzzle.rename_collides(&symbol, &params.new_name) {
            return Err(HandlerError::refusal(
                "rename.nameCollision",
                json!({ "newName": params.new_name }),
                "new name collides with an existing symbol",
            ));
        }

        let edits: Vec<Value> = sorted_entries(&params.documents)
            .into_iter()
            .filter_map(|(key, doc)| {
                let puzzle = parse_document(&doc.text).puzzle?;
                let edits: Vec<Value> = puzzle
                    .rename_ranges(&symbol, key == &params.position_document_uri)
                    .into_iter()
                    .map(|range| json!({ "range": range, "newText": params.new_name }))
                    .collect();
                (!edits.is_empty()).then(|| {
                    json!({
                        "documentUri": doc.uri,
                        "version": doc.version,
                        "textEdits": edits,
                    })
                })
            })
            .collect();
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
        match puzzle::validate_edits(&params.document.text, &edits) {
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

    fn lookup(&self, params: Value) -> Result<Value, HandlerError> {
        let params: LookupParams = parse_params(params)?;
        if params.language_id != LANGUAGE_ID {
            return Err(HandlerError::lpp(
                "invalidLanguage",
                json!({ "languageId": params.language_id }),
                format!(
                    "language '{}' is not served by this provider",
                    params.language_id
                ),
            ));
        }
        if matches!(params.limit, Some(0)) {
            return Err(HandlerError::invalid_params());
        }
        let scope = match params.within.as_ref() {
            None => LookupScope::All,
            Some(within) => match within.kind.as_str() {
                "callable" => LookupScope::Callable(within.value.clone()),
                "enum" => LookupScope::Enum(within.value.clone()),
                "settings" => LookupScope::Settings(within.value.clone()),
                _ => return Err(HandlerError::invalid_params()),
            },
        };
        let entries = puzzle::lookup(
            params.query.as_deref().unwrap_or_default(),
            params.kind.as_deref(),
            &scope,
            params.limit.unwrap_or(DEFAULT_LOOKUP_LIMIT) as usize,
        )
        .ok_or_else(|| {
            HandlerError::refusal(
                "lookup.unknownWithin",
                json!({ "within": params.within }),
                "within selector names no known scope",
            )
        })?;
        Ok(json!({ "entries": entries }))
    }
}

/// The provider-side bound applied when a request omits `limit`.
const DEFAULT_LOOKUP_LIMIT: u32 = 20;

/// Reads `acceptedArtifactFormats` from `lpp/compile` params. Valid only
/// in LPP 1.4-or-later sessions, as a non-empty array of strings.
fn accepted_artifact_formats(
    server: &Server,
    params: &Value,
) -> Result<Option<Vec<String>>, HandlerError> {
    let Some(field) = params.get("acceptedArtifactFormats") else {
        return Ok(None);
    };
    if !server.since(4) {
        return Err(HandlerError::invalid_params());
    }
    match serde_json::from_value::<Vec<String>>(field.clone()) {
        Ok(formats) if !formats.is_empty() => Ok(Some(formats)),
        _ => Err(HandlerError::invalid_params()),
    }
}

fn select_artifact_format(accepted: Option<Vec<String>>) -> Result<&'static str, HandlerError> {
    match accepted {
        None => Ok(ARTIFACT_FORMAT),
        // The client's list is in preference order: the first accepted format
        // we support wins.
        Some(accepted) => accepted
            .iter()
            .find_map(|a| {
                COMPILE_ARTIFACT_FORMATS
                    .into_iter()
                    .find(|s| *s == a.as_str())
            })
            .ok_or_else(|| {
                HandlerError::refusal(
                    "compile.artifactFormatUnsupported",
                    json!({}),
                    "none of the accepted artifact formats is supported",
                )
            }),
    }
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Document-set entries in key order.
fn sorted_entries(map: &HashMap<String, Document>) -> Vec<(&String, &Document)> {
    let mut entries: Vec<(&String, &Document)> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

/// Documents in document-set key order.
fn sorted_docs(map: &HashMap<String, Document>) -> Vec<&Document> {
    sorted_entries(map)
        .into_iter()
        .map(|(_, doc)| doc)
        .collect()
}

/// Check and parse every document in document-set key order.
fn parsed_set(
    map: &HashMap<String, Document>,
) -> Result<Vec<(&Document, ParseOutput)>, HandlerError> {
    sorted_docs(map)
        .into_iter()
        .map(|doc| {
            check_document(doc)?;
            Ok((doc, parse_document(&doc.text)))
        })
        .collect()
}

fn check_document(doc: &Document) -> Result<(), HandlerError> {
    if doc.language_id != LANGUAGE_ID {
        return Err(HandlerError::lpp(
            "invalidLanguage",
            json!({ "languageId": doc.language_id }),
            format!(
                "language '{}' is not served by this provider",
                doc.language_id
            ),
        ));
    }
    if doc.version < 0 {
        return Err(HandlerError::lpp(
            "invalidDocument",
            json!({ "uri": doc.uri, "reason": "invalidVersion" }),
            "document version must be a non-negative integer",
        ));
    }
    Ok(())
}

/// `{ uri, version, <key>: <value> }` — the per-document view shape shared by
/// check, compile, and symbols.
fn document_view(doc: &Document, key: &str, value: Value) -> Value {
    let mut view = json!({ "uri": doc.uri, "version": doc.version });
    view[key] = value;
    view
}

fn location_json(uri: &str, range: Range) -> Value {
    json!({ "uri": uri, "range": range })
}

/// The shared definition/references/rename preamble: document checks, then
/// the symbol under `position`, or the method's `noSymbolAtPosition` refusal.
fn symbol_at_position(
    doc: &Document,
    position: Position,
    refusal_code: &'static str,
    uri: &str,
) -> Result<(Puzzle, Symbol), HandlerError> {
    check_document(doc)?;
    let src = SourceText::new(&doc.text);
    let Some(byte) = src.byte_of(position) else {
        return Err(HandlerError::lpp(
            "invalidPosition",
            json!({ "uri": uri, "position": position }),
            "position outside document",
        ));
    };
    let no_symbol =
        || HandlerError::refusal(refusal_code, json!({ "uri": uri }), "no symbol at position");
    let puzzle = parse_document(&doc.text)
        .into_puzzle()
        .ok_or_else(no_symbol)?;
    let symbol = puzzle.symbol_at(&src, byte).ok_or_else(no_symbol)?;
    Ok((puzzle, symbol))
}
