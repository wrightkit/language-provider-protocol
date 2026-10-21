use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::puzzle::{Position, Range};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitParams {
    pub protocol_version: String,
    #[allow(dead_code)]
    pub client_info: Option<ClientInfo>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub(crate) struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Document {
    pub uri: String,
    pub language_id: String,
    pub version: i64,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectEntry {
    pub uri: String,
    pub language_id: String,
    pub version: i64,
    pub kind: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocsParams {
    pub documents: Option<HashMap<String, Document>>,
    pub entry: Option<ProjectEntry>,
    #[allow(dead_code)]
    pub project_root: Option<String>,
    #[allow(dead_code)]
    pub locale: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Artifact {
    pub format: String,
    pub content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReconstructParams {
    pub artifact: Artifact,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PositionParams {
    pub document: Document,
    pub position: Position,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferencesParams {
    pub document: Document,
    pub position: Position,
    pub include_declaration: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameParams {
    pub documents: HashMap<String, Document>,
    pub position_document_uri: String,
    pub position: Position,
    pub new_name: String,
    #[allow(dead_code)]
    pub project_root: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LookupWithin {
    pub kind: String,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LookupParams {
    pub language_id: String,
    pub query: Option<String>,
    pub kind: Option<String>,
    pub within: Option<LookupWithin>,
    #[allow(dead_code)]
    pub locale: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolveIdsParams {
    pub language_id: String,
    pub calls: Option<Vec<String>>,
    pub enums: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidateEditsParams {
    pub document: Document,
    pub edits: Vec<TextEdit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextEdit {
    pub range: Range,
    pub new_text: String,
}
