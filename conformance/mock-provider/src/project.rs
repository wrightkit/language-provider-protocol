use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use lpp_conformance_common::{file_uri_to_path, path_to_file_uri};
use serde_json::json;

use crate::rpc::HandlerError;
use crate::server::{LANGUAGE_ID, Server};
use crate::wire::{DocsParams, Document, ProjectEntry};

pub(crate) struct LoadedEntry {
    pub(crate) uri: String,
    pub(crate) inputs: Vec<String>,
}

pub(crate) fn documents_for_request(
    server: &Server,
    params: DocsParams,
    method: &str,
) -> Result<(HashMap<String, Document>, Option<LoadedEntry>), HandlerError> {
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
            let (documents, loaded) = Loader {
                server,
                entry: &entry,
            }
            .run()?;
            Ok((documents, Some(loaded)))
        }
        _ => Err(HandlerError::invalid_params()),
    }
}

#[derive(Debug, Clone, Copy)]
enum Target {
    File,
    Directory,
}

struct Loader<'a> {
    server: &'a Server,
    entry: &'a ProjectEntry,
}

impl Loader<'_> {
    fn invalid(&self, reason: &str, message: impl Into<String>) -> HandlerError {
        HandlerError::lpp(
            "invalidEntry",
            json!({ "entryUri": self.entry.uri, "reason": reason }),
            message,
        )
    }

    fn fail(&self, reason: &str, uri: Option<&str>, message: impl Into<String>) -> HandlerError {
        let mut details = json!({ "entryUri": self.entry.uri, "reason": reason });
        if let Some(uri) = uri {
            details["uri"] = json!(uri);
        }
        HandlerError::lpp("projectLoadFailed", details, message)
    }

    fn run(self) -> Result<(HashMap<String, Document>, LoadedEntry), HandlerError> {
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

        let mut traversed = vec![path.clone()];
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
                traversed.push(selected.clone());
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

        let entries: Vec<_> = std::fs::read_dir(&root)
            .map_err(|error| {
                self.fail(
                    "projectDirectoryUnreadable",
                    None,
                    format!("project directory could not be read: {error}"),
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                self.fail(
                    "projectDirectoryUnreadable",
                    None,
                    format!("project directory could not be read: {error}"),
                )
            })?;
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let path = entry.path();
            if path != entry_path {
                if path.extension().is_none_or(|ext| ext != "xdl") {
                    continue;
                }
                let is_dir = entry
                    .file_type()
                    .map_err(|error| {
                        self.fail(
                            "sourceFileUnreadable",
                            path_to_file_uri(&path).as_deref(),
                            format!("source file type could not be read: {error}"),
                        )
                    })?
                    .is_dir();
                if is_dir {
                    continue;
                }
            }
            paths.push(path);
        }
        paths.sort();

        traversed.push(root.clone());
        traversed.extend(paths.iter().cloned());
        let mut documents = HashMap::new();
        for path in paths {
            let unresolved_uri = path_to_file_uri(&path);
            let path = std::fs::canonicalize(&path).map_err(|error| {
                self.fail(
                    "sourceFileUnreadable",
                    unresolved_uri.as_deref(),
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
        let inputs = documents
            .keys()
            .cloned()
            .chain(declared_paths(&traversed).filter_map(|path| path_to_file_uri(&path)))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok((
            documents,
            LoadedEntry {
                uri: canonical_entry_uri,
                inputs,
            },
        ))
    }
}

/// Each traversed path plus every symbolic link followed while resolving it,
/// including links in a chain and linked directory components, so a
/// retargeted link shows up as a changed declared input.
fn declared_paths(traversed: &[PathBuf]) -> impl Iterator<Item = PathBuf> + '_ {
    traversed.iter().flat_map(|path| {
        let mut declared = vec![path.clone()];
        let mut hops = 0;
        resolve(path, &mut declared, &mut hops);
        declared
    })
}

const MAX_LINK_HOPS: u32 = 40;

fn resolve(path: &Path, links: &mut Vec<PathBuf>, hops: &mut u32) -> PathBuf {
    let mut resolved = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let next = resolved.join(name);
                match std::fs::read_link(&next) {
                    Ok(target) if *hops < MAX_LINK_HOPS => {
                        *hops += 1;
                        links.push(next);
                        resolved = resolve(&resolved.join(target), links, hops);
                    }
                    _ => resolved = next,
                }
            }
            Component::ParentDir => {
                resolved.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    resolved
}
