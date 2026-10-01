//! Project loading: resolving an `entry` target (LPP 1.1 file, LPP 1.2
//! directory) into the in-memory document set that `lpp/check` and
//! `lpp/compile` consume.

use std::collections::HashMap;
use std::path::PathBuf;

use lpp_conformance_common::{file_uri_to_path, path_to_file_uri};
use serde_json::json;

use crate::rpc::HandlerError;
use crate::server::{LANGUAGE_ID, Server};
use crate::wire::{DocsParams, Document, ProjectEntry};

/// Resolve the document set for a `lpp/check`/`lpp/compile`-style request:
/// either the inline `documents` map or a project `entry` to load.
/// Returns the set and the canonical entry URI when loaded from a project.
pub(crate) fn documents_for_request(
    server: &Server,
    params: DocsParams,
    method: &str,
) -> Result<(HashMap<String, Document>, Option<String>), HandlerError> {
    match (params.documents, params.entry) {
        (Some(documents), None) => Ok((documents, None)),
        (None, Some(entry)) => {
            if !server.since(1) {
                return Err(HandlerError::invalid_params());
            }
            if !server.capability("projectLoading") {
                return Err(HandlerError::lpp(
                    "capabilityUnavailable",
                    json!({ "capability": "projectLoading", "method": method }),
                    "capability 'projectLoading' is not available",
                ));
            }
            let (documents, entry_uri) = Loader {
                server,
                entry: &entry,
            }
            .run()?;
            Ok((documents, Some(entry_uri)))
        }
        _ => Err(HandlerError::invalid_params()),
    }
}

#[derive(Debug, Clone, Copy)]
enum Target {
    File,
    Directory,
}

/// Project loading tied to one request's `ProjectEntry`. The shared error
/// constructors live here so call sites name only the reason and message.
struct Loader<'a> {
    server: &'a Server,
    entry: &'a ProjectEntry,
}

impl Loader<'_> {
    /// `invalidEntry`: the request's entry object failed validation.
    fn invalid(&self, reason: &str, message: impl Into<String>) -> HandlerError {
        HandlerError::lpp(
            "invalidEntry",
            json!({ "entryUri": self.entry.uri, "reason": reason }),
            message,
        )
    }

    /// `projectLoadFailed`: filesystem resolution failed. `uri` is the
    /// concrete file URI when one is known.
    fn fail(&self, reason: &str, uri: Option<&str>, message: impl Into<String>) -> HandlerError {
        let mut details = json!({ "entryUri": self.entry.uri, "reason": reason });
        if let Some(uri) = uri {
            details["uri"] = json!(uri);
        }
        HandlerError::lpp("projectLoadFailed", details, message)
    }

    fn run(self) -> Result<(HashMap<String, Document>, String), HandlerError> {
        let entry = self.entry;
        if entry.version < 0 {
            return Err(self.invalid(
                "invalidVersion",
                "project entry version must be a non-negative integer",
            ));
        }
        let target = match entry.kind.as_deref() {
            None | Some("file") => Target::File,
            Some("directory") if self.server.since(2) => Target::Directory,
            Some("directory") => {
                return Err(self.invalid(
                    "unsupportedKind",
                    "directory project targets require protocol version 1.2",
                ));
            }
            Some(_) => {
                return Err(self.invalid("unsupportedKind", "project target kind is not supported"));
            }
        };
        if entry.language_id != LANGUAGE_ID {
            return Err(self.invalid(
                "unsupportedLanguage",
                format!(
                    "language '{}' is not served by this provider",
                    entry.language_id
                ),
            ));
        }
        let Some(path) = file_uri_to_path(&entry.uri) else {
            return Err(self.invalid(
                "unsupportedUri",
                "project entry must be an absolute file URI",
            ));
        };

        let path = std::fs::canonicalize(&path).map_err(|_| {
            let (reason, message) = match target {
                Target::File => ("entryNotFound", "project entry could not be loaded"),
                Target::Directory => ("targetNotFound", "project target could not be loaded"),
            };
            self.fail(reason, Some(&entry.uri), message)
        })?;

        let (root, entry_path) = match target {
            Target::File => {
                if !path.is_file() {
                    return Err(self.fail(
                        "entryNotFile",
                        Some(&entry.uri),
                        "project entry is not a file",
                    ));
                }
                (
                    path.parent().expect("a file has a parent").to_path_buf(),
                    path,
                )
            }
            Target::Directory => {
                if !path.is_dir() {
                    return Err(self.fail(
                        "targetNotDirectory",
                        Some(&entry.uri),
                        "project target is not a directory",
                    ));
                }
                let selected = path.join("entry.xdl");
                if !selected.is_file() {
                    return Err(self.fail(
                        "defaultEntryNotFound",
                        Some(&entry.uri),
                        "project directory has no default entry",
                    ));
                }
                let selected = selected.canonicalize().map_err(|error| {
                    let uri = path_to_file_uri(&selected).unwrap_or_else(|| entry.uri.clone());
                    self.fail(
                        "defaultEntryUnreadable",
                        Some(&uri),
                        format!("default project entry could not be resolved: {error}"),
                    )
                })?;
                (path, selected)
            }
        };
        let canonical_entry_uri = path_to_file_uri(&entry_path).ok_or_else(|| {
            self.fail(
                "sourceIdentityUnavailable",
                None,
                "project entry has no stable URI",
            )
        })?;

        // The project is the `.xdl` files in the selected entry's directory.
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
            .map_err(|error| {
                self.fail(
                    "projectDirectoryUnreadable",
                    None,
                    format!("project directory could not be read: {error}"),
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path == &entry_path || path.extension().is_some_and(|ext| ext == "xdl"))
            .collect();
        paths.sort();

        let mut documents = HashMap::new();
        for path in paths {
            let path = std::fs::canonicalize(&path).map_err(|error| {
                self.fail(
                    "sourceFileUnreadable",
                    None,
                    format!("source file could not be resolved: {error}"),
                )
            })?;
            let uri = path_to_file_uri(&path).ok_or_else(|| {
                self.fail(
                    "sourceIdentityUnavailable",
                    None,
                    "source file has no stable URI",
                )
            })?;
            let text = std::fs::read_to_string(&path).map_err(|error| {
                self.fail(
                    "sourceFileUnreadable",
                    Some(&uri),
                    format!("source file could not be read: {error}"),
                )
            })?;
            documents.insert(
                uri.clone(),
                Document {
                    uri,
                    language_id: LANGUAGE_ID.to_string(),
                    version: entry.version,
                    text,
                },
            );
        }
        Ok((documents, canonical_entry_uri))
    }
}
