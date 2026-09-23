# LPP v1 — Initialization and project requests

[← LPP v1 specification index](README.md)

## 7. lpp/initialize

Initialization and capability negotiation. The client MUST send
`lpp/initialize` as the first message of a session.

### 7.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "lpp/initialize",
  "params": {
    "protocolVersion": "1.0",
    "clientInfo": { "name": "wright", "version": "0.2.0" }
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `protocolVersion` | string | The protocol version the client wants to speak: `"1.0"`, `"1.1"`, `"1.2"`, or `"1.3"`. |
| `clientInfo` | object, OPTIONAL | `{ "name": string, "version": string }` identifying the client. |

### 7.2 Result

```json
{
  "protocolVersion": "1.0",
  "serverInfo": { "name": "lpp-mock-provider", "version": "0.1.0" },
  "languages": [
    { "id": "x-demo-lang", "extensions": ["xdl"] }
  ],
  "capabilities": {
    "check": true,
    "compile": true,
    "reconstruct": true,
    "symbols": true,
    "definition": true,
    "references": true,
    "rename": true,
    "editValidation": true
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `protocolVersion` | string | The protocol version the provider will speak: `"1.0"`, `"1.1"`, `"1.2"`, or `"1.3"`. |
| `serverInfo` | object | `{ "name": string, "version": string }` identifying the provider. |
| `languages` | array | One entry per source language the provider serves. |
| `capabilities` | object | One boolean field per capability. LPP 1.0 requires the eight fields listed below; LPP 1.1, 1.2, and 1.3 additionally require `projectLoading`; LPP 1.3 also defines `sourceIdentity`. |

Each language entry: `{ "id": string, "extensions": [string] }`. `extensions`
is the list of file extensions the provider associates with the language,
written WITHOUT a leading dot and in lowercase (for example `["xdl"]`,
`["opy"]`). The list MAY be empty if the language has no conventional
extension.

### 7.3 Capability negotiation

* Capability ids and their methods:

| Capability | Methods | Purpose |
| --- | --- | --- |
| `check` | `lpp/check` | Produce diagnostics for documents. |
| `compile` | `lpp/compile` | Compile a document set to a Workshop artifact. |
| `reconstruct` | `lpp/reconstruct` | Reconstruct source from a Workshop artifact. |
| `symbols` | `lpp/symbols` | List symbols in documents. |
| `definition` | `lpp/definition` | Resolve the definition at a position. |
| `references` | `lpp/references` | Find references to the symbol at a position. |
| `rename` | `lpp/rename` | Compute source edits for a semantic rename. |
| `editValidation` | `lpp/validateEdits` | Validate a set of source edits against a document. |
| `projectLoading` | `lpp/check`, `lpp/compile` | Accept a client-selected entry or directory target and load its filesystem-backed source project. LPP 1.1, 1.2, and 1.3. |
| `sourceIdentity` | `lpp/compile` | Return the provider-selected primary source identity for an entry-based compile result. LPP 1.3. |

* The provider MUST set each capability to `true` only if it fully implements
  the corresponding method(s).
* The client MUST NOT invoke a method whose capability was advertised as
  `false` or absent. If it does, the provider MUST respond with an LPP error
  of kind `capabilityUnavailable` with `details.capability` and
  `details.method`.
* Capabilities are independent: a provider MAY advertise any subset.
* Capability ids are a closed set for each LPP minor version. New capability
  ids can only be introduced through a new protocol version or a negotiated
  additive revision (see [Section 19](#19-protocol-evolution-and-version-negotiation)).

### 7.4 Protocol version mismatch

If the provider does not support the client's `protocolVersion`, it MUST
respond with an LPP error of kind `protocolVersionMismatch` whose
`details.supportedProtocolVersions` lists every protocol version the provider
supports. A provider supporting LPP 1.1 SHOULD continue to support LPP 1.0
when practical:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "unsupported protocol version 0.9",
    "data": {
      "lpp": {
        "kind": "protocolVersionMismatch",
        "details": { "supportedProtocolVersions": ["1.0"] }
      }
    }
  }
}
```

The client then decides whether to terminate the session or restart with a
supported version. LPP 1.0 clients MUST send `"1.0"`; clients using file-entry
project loading MUST send `"1.1"`, `"1.2"`, or `"1.3"`; clients using directory
targets MUST send `"1.2"` or `"1.3"`. A client that requires the
`sourceIdentity` capability MUST request `"1.3"` and require the provider to
advertise `sourceIdentity: true` before using entry-based compile results.

## 8. Common request parameters

Document-scoped methods share this parameter shape:

| Field | Type | Methods | Description |
| --- | --- | --- | --- |
| `documents` | DocumentSet | `check`, `compile`, `symbols`, `rename` | The documents to operate on. |
| `entry` | Project entry | `check`, `compile` in LPP 1.1, 1.2, and 1.3 | Alternative to `documents`; asks the provider to load the source closure from the selected entry or directory target. |
| `document` | Document | `definition`, `references`, `validateEdits` | The single document to operate on. |
| `projectRoot` | string, OPTIONAL | `check`, `compile`, `symbols`, `rename` | URI identifying the project the documents belong to. Purely informational in v1; providers MUST accept and MAY use it. |

### 8.1 Entry-based project requests (LPP 1.1, 1.2, and 1.3)

In LPP 1.1, 1.2, and 1.3, `lpp/check` and `lpp/compile` accept either `documents` or
`entry`, but not both. An `entry` request is available only when the provider
accepted protocol version `1.1`, `1.2`, or `1.3` and advertised
`projectLoading: true`.
The optional `projectRoot` field remains legal and is informational; the
provider accepts it but determines the effective project root and source
closure from the entry and the source language's rules.

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "lpp/check",
  "params": {
    "entry": {
      "uri": "file:///project/main.opy",
      "languageId": "opy",
      "version": 7
    }
  }
}
```

For a file target, the provider MUST load the entry and every additional
source file required by the source language's project rules, then perform the
requested operation on that complete source closure. It MUST NOT require the
client to list those files in advance. The provider MUST read only the
filesystem project identified by the entry and MUST NOT treat the client's
working directory as a project root unless that is the source language's
documented rule.

The result uses the normal `lpp/check` or `lpp/compile` shape. It MUST include
diagnostics for every loaded source document, including documents that contain
no diagnostics. Every filesystem-loaded document result MUST use the entry's
`version`; this identifies the client-selected snapshot and is not a
filesystem content hash. The provider MUST preserve stable source identity by
returning the canonical URI it uses for each loaded file. It MUST fail instead
of returning a partial result when the entry or a required source file cannot
be loaded.

An entry with an unsupported URI or language produces an LPP error of kind
`invalidEntry`. A missing or unreadable entry or required source file produces
an LPP error of kind `projectLoadFailed`. The `details` object MUST contain
`entryUri` and a provider-defined `reason`; a required-file failure SHOULD
also include the affected `uri`.

### 8.2 Directory project requests (LPP 1.2 and 1.3)

LPP 1.2 and 1.3 extend the `entry` object with `kind: "directory"`:

```json
{
  "entry": {
    "uri": "file:///project",
    "languageId": "opy",
    "version": 7,
    "kind": "directory"
  }
}
```

The URI MUST identify an absolute filesystem directory. The provider owns
selection of the effective project entry, project root, and source closure
according to the source language's rules. The client MUST NOT enumerate files,
parse project manifests, or select an entry file to emulate this behavior.
Providers MUST return canonical source identities for every loaded document and
MUST fail with `projectLoadFailed` if the directory or its language-owned
default entry cannot be loaded. A `kind` of `"file"` has the same semantics as
the LPP 1.1 entry request. LPP 1.1 clients omit `kind` and therefore always
request file-entry behavior. A directory target sent in an LPP 1.1 session
MUST be rejected as `invalidEntry`; the provider MUST NOT perform directory
entry selection. Any other `kind` value MUST also be rejected as
`invalidEntry`.

