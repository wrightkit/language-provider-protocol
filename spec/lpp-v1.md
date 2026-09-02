# Language Provider Protocol (LPP): Version 1

| | |
| --- | --- |
| Protocol versions | `1.0`, `1.1` |
| Status | Normative for protocol major version 1 |
| Transport | JSON-RPC 2.0 over stdio, newline-delimited framing |
| Conformance | `conformance/fixtures/v1/` + `conformance/runner` + `conformance/mock-provider` |
| Client integration | wrightkit/wright#142 (Wright client/runtime) |

## 1. Introduction

The Language Provider Protocol (LPP) defines the contract between a **client**
(tooling such as a CLI, language services, or agent tooling) and a
**provider**: a long-running process that understands one or more source
languages used to author Overwatch Workshop content (OPY, OSTW, or any
third-party language).

LPP is a **process boundary**. A client communicates with a provider through
JSON-RPC 2.0 messages carrying source text, positions, ranges, diagnostics,
and source-oriented edits. The protocol MUST NOT expose provider-internal
representations (lexer/parser state, ASTs, HIR, or compiler IR) and MUST NOT
expose Wright or workshop-rs internal representations. Compile results cross
the boundary only as opaque **Workshop artifacts** (see
[Section 10](#10-lppcompile)).

LPP is intentionally language-agnostic: nothing in the wire format assumes a
particular source language, a particular implementation language for the
provider, or a particular client. The protocol is designed to be implementable
outside Rust; no Rust type or encoding appears in the wire format.

### 1.1 Goals

* Define a small, versioned protocol supporting the current ecosystem needs:
  checking, compilation to a Workshop artifact, reconstruction from a Workshop
  artifact, symbols/definition/references, semantic rename, and edit
  validation.
* Make source identity, positions/ranges, edit versions, errors, and refusals
  explicit.
* Keep every capability independently optional and negotiated at
  initialization.
* Provide machine-testable conformance fixtures so that a provider in any
  language can be verified.

### 1.2 Non-goals

* Dynamic-library/FFI plugin ABIs.
* Remote provider discovery or provider marketplaces.
* Generic compiler/type-system/AST APIs.
* Defining the semantics of any source language or of any Workshop artifact
  format (the artifact *envelope* is defined here; formats are owned by the
  ecosystem, see [Section 10.2](#102-artifact-formats)).
* Freezing a portable Workshop semantic IR. LPP does not interpret artifact
  content.

## 2. Conventions and terminology

### 2.1 Keywords

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY, and OPTIONAL in this document are to be interpreted as
described in RFC 2119.

### 2.2 Terminology

* **Client**: the process that spawns and communicates with a provider.
* **Provider**: the long-running process implementing LPP for one or more
  source languages.
* **Session**: one provider process lifetime, from spawn until exit.
* **Document**: a unit of source text identified by a URI, tagged with a
  language id and a version.
* **Document set**: a collection of documents supplied with a request. LPP is
  stateless with respect to document contents: every request carries the text
  it operates on.
* **Workshop artifact**: an opaque envelope (format id + content) produced by
  compilation and consumed by reconstruction.
* **Capability**: an independently optional protocol feature advertised by the
  provider during initialization.
* **Refusal**: a well-formed, machine-readable decline of a request that the
  provider understood. Refusals are a normal outcome, distinct from errors.

## 3. Transport

### 3.1 Process model

The client MUST spawn the provider as a child process. The provider MUST
communicate over its standard input and standard output. The provider's
standard error is reserved for human-readable logging; it MUST NOT be used for
protocol messages.

### 3.2 Framing

Each protocol message is exactly one JSON-RPC 2.0 message serialized as a
single line of UTF-8 text terminated by LF (`0x0A`). Messages MUST NOT contain
raw newline characters; newlines inside JSON strings are escaped as `\n`.

* Writers MUST emit LF line terminators and MUST flush after each message.
* Readers MUST accept both LF and CRLF line terminators.
* Readers MUST ignore empty lines.

The client MAY terminate the provider at any time (for example by closing its
standard input or by sending a signal). When the provider's standard input
reaches end-of-file, the provider MUST exit promptly with status 0. After
sending an `lpp/shutdown` response the provider MUST exit with status 0.
Providers SHOULD also exit promptly with status 0 on SIGTERM and SIGINT.

### 3.3 Encoding and limits

All text is UTF-8. Clients and providers MUST accept messages of at least
16 MiB.

## 4. JSON-RPC conformance

Messages MUST be valid JSON-RPC 2.0 requests and responses
(https://www.jsonrpc.org/specification), with the following LPP-specific
rules:

* **Requests**: every request has an `id` (integer or string), a `method`
  string, and a `params` object. The `params` field is REQUIRED in all LPP v1
  methods; where a method takes no parameters, the client MUST send an empty
  object `{}` and the provider MUST ignore it.
* **No notifications**: LPP v1 defines no notifications. A message without an
  `id`, or with a null `id`, is a protocol violation; the provider MUST
  respond with an LPP error of kind `invalidRequest` and
  `details.reason` = `notificationNotSupported`, with `id` null in the
  response. (This is an explicit deviation from JSON-RPC notification
  semantics, chosen so that client mistakes are always observable.)
* **No batches**: clients MUST NOT send batch messages. If a provider receives
  an array message, it MUST respond with the standard JSON-RPC error
  `-32600` ("Invalid Request") and `id` null.
* **Processing order**: the provider MUST process messages in the order
  received and MUST emit responses in that same order.
* **Version field**: every request and response MUST carry `"jsonrpc": "2.0"`.

### 4.1 Standard JSON-RPC errors

The provider MUST use the standard JSON-RPC error codes, with no LPP error
data attached:

| Code | Name | When |
| --- | --- | --- |
| `-32700` | Parse error | A message line is not valid JSON. Response `id` is null. |
| `-32600` | Invalid Request | The message is not a valid JSON-RPC request (wrong `jsonrpc` value, missing `method`, non-object message, batch). |
| `-32601` | Method not found | The method name is not a known LPP v1 method. |
| `-32602` | Invalid params | The `params` value does not match the method's schema (wrong types or missing required fields). |
| `-32603` | Internal error | The provider failed internally. |

## 5. Session lifecycle

```text
client spawns provider
        |
        v
lpp/initialize  -- success --> ready
        |                       |
        | error                 | lpp/check, lpp/compile, ...
        |                       |
        v                       v
  exit (client decides)    lpp/shutdown or EOF
                                |
                                v
                        provider exits (status 0)
```

* Before a successful `lpp/initialize`, the provider MUST NOT process any
  other method; any other request MUST be answered with an LPP error of kind
  `invalidRequest` and `details.reason` = `notInitialized`.
* A second `lpp/initialize` after a successful one MUST be answered with an
  LPP error of kind `invalidRequest` and `details.reason` =
  `alreadyInitialized`.
* After the `lpp/initialize` response, the session is ready. There is no
  separate "initialized" notification in LPP v1.
* Requests arriving after `lpp/shutdown` was sent MAY be ignored by the
  provider (the client MUST NOT send any).

## 6. Common data types

All positions and ranges use LSP conventions.

### 6.1 Position

```json
{ "line": 0, "character": 0 }
```

* `line`: 0-based line index.
* `character`: 0-based character offset within the line, measured in **UTF-16
  code units** (this matches the de-facto editor protocol convention; a
  position inside a supplementary-plane character is not valid).

A position is valid if `line` is within the document's line count and
`character` is within the UTF-16 length of that line (a position at the end of
a line is valid).

### 6.2 Range

```json
{ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } }
```

A half-open interval: `start` is inclusive, `end` is exclusive. `start` MUST
not be after `end`.

### 6.3 TextEdit

```json
{ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } }, "newText": "target" }
```

Replaces the text in `range` with `newText`. All edits are expressed in the
coordinates of the original document text as received by the provider.

### 6.4 Document

```json
{
  "uri": "file:///project/puzzle.xdl",
  "languageId": "x-demo-lang",
  "version": 3,
  "text": "puzzle clean { ... }"
}
```

* `uri`: a string URI (RFC 3986). URIs are compared by exact string equality
  for identity; clients and providers MUST NOT rely on normalization.
* `languageId`: the language id as declared by the provider in
  `lpp/initialize`. REQUIRED.
* `version`: a non-negative integer maintained by the client; it MUST increase
  by at least 1 whenever the client changes the document text. Because LPP is
  stateless, the text always travels with the request; the version lets both
  sides tag results and detect stale client bookkeeping.
* `text`: the full current source text.

### 6.5 DocumentSet

```json
{
  "file:///project/puzzle.xdl": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." }
}
```

An object mapping document URI to Document. For document-supplied requests, the
client MUST include every document the request may need; the provider MUST NOT
assume any document exists outside the set and MUST NOT return edits for
documents it did not receive. LPP 1.1 entry-based `check` and `compile`
requests are the explicit filesystem-loading exception defined in
[Section 8.1](#81-entry-based-project-requests-lpp-11).

### 6.6 Diagnostic

```json
{
  "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } },
  "severity": "error",
  "code": "x-demo/duplicate-op",
  "message": "duplicate op name 'double'",
  "source": "x-demo-lang"
}
```

* `severity`: one of `error`, `warning`, `info`, `hint`.
* `code`: OPTIONAL provider-defined diagnostic code (string).
* `message`: REQUIRED human-readable message.
* `source`: OPTIONAL string naming the diagnostic origin.

### 6.7 Location

```json
{ "uri": "file:///project/puzzle.xdl", "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } } }
```

### 6.8 Symbol

```json
{
  "name": "double",
  "kind": "op",
  "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } },
  "selectionRange": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } }
}
```

* `kind`: a provider-defined string (for example `puzzle`, `op`, `function`,
  `rule`, `variable`). LPP does not define a fixed kind vocabulary.
* `selectionRange`: OPTIONAL; defaults to `range` when absent.

### 6.9 WorkshopArtifact

```json
{ "format": "x-demo/puzzle-eval-v1", "content": "{\"name\":\"clean\",...}" }
```

* `format`: a REQUIRED string identifying the artifact format. See
  [Section 10.2](#102-artifact-formats).
* `content`: a REQUIRED string holding the artifact payload in that format.
  The payload is UTF-8 text; LPP never interprets it.

### 6.10 Project entry

A project entry identifies the source file selected by the client for a
provider-owned filesystem project load:

```json
{
  "uri": "file:///project/main.opy",
  "languageId": "opy",
  "version": 7
}
```

* `uri`: a REQUIRED absolute `file` URI. A client that starts with a local
  path MUST resolve it to an absolute file URI before sending it.
* `languageId`: the REQUIRED language id selected by the client. It MUST be
  one of the languages advertised by the provider.
* `version`: a REQUIRED non-negative integer identifying the client-selected
  filesystem snapshot for this request. It is echoed in every source result.
  It is not a filesystem content hash and does not provide cross-request stale
  detection.

The entry identifies the user's selected source target only. The provider
determines the effective project root and source closure according to the
source language's rules; the client MUST NOT infer or supply that closure.

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
| `protocolVersion` | string | The protocol version the client wants to speak: `"1.0"` or `"1.1"`. |
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
| `protocolVersion` | string | The protocol version the provider will speak: `"1.0"` or `"1.1"`. |
| `serverInfo` | object | `{ "name": string, "version": string }` identifying the provider. |
| `languages` | array | One entry per source language the provider serves. |
| `capabilities` | object | One boolean field per capability. LPP 1.0 requires the eight fields listed below; LPP 1.1 additionally requires `projectLoading`. |

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
| `projectLoading` | `lpp/check`, `lpp/compile` | Accept a client-selected entry and load its filesystem-backed source project. LPP 1.1 only. |

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
supported version. LPP 1.0 clients MUST send `"1.0"`; clients using the
project-loading extension MUST send `"1.1"`.

## 8. Common request parameters

Document-scoped methods share this parameter shape:

| Field | Type | Methods | Description |
| --- | --- | --- | --- |
| `documents` | DocumentSet | `check`, `compile`, `symbols`, `rename` | The documents to operate on. |
| `entry` | Project entry | `check`, `compile` in LPP 1.1 | Alternative to `documents`; asks the provider to load the source closure from the selected entry. |
| `document` | Document | `definition`, `references`, `validateEdits` | The single document to operate on. |
| `projectRoot` | string, OPTIONAL | `check`, `compile`, `symbols`, `rename` | URI identifying the project the documents belong to. Purely informational in v1; providers MUST accept and MAY use it. |

### 8.1 Entry-based project requests (LPP 1.1)

In LPP 1.1, `lpp/check` and `lpp/compile` accept either `documents` or
`entry`, but not both. An `entry` request is available only when the provider
accepted protocol version `1.1` and advertised `projectLoading: true`.
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

The provider MUST load the entry and every additional source file required by
the source language's project rules, then perform the requested operation on
that complete source closure. It MUST NOT require the client to list those
files in advance. The provider MUST read only the filesystem project
identified by the entry and MUST NOT treat the client's working directory as a
project root unless that is the source language's documented rule.

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

## 9. lpp/check

Produce diagnostics for a set of documents. The provider MUST parse and
analyze every document in the set and MUST report all diagnostics found.

### 9.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "lpp/check",
  "params": {
    "documents": { "file:///project/puzzle.xdl": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." } },
    "projectRoot": "file:///project"
  }
}
```

### 9.2 Result

```json
{
  "documents": [
    {
      "uri": "file:///project/puzzle.xdl",
      "version": 3,
      "diagnostics": [ { "range": { "start": { "line": 7, "character": 31 }, "end": { "line": 7, "character": 37 } }, "severity": "error", "code": "x-demo/unresolved-op", "message": "unresolved op reference 'triple'", "source": "x-demo-lang" } ]
    }
  ]
}
```

* `documents`: one entry per input document, in the same order the client
  would observe from the request object (providers SHOULD use a
  deterministic order; the mock provider orders entries by URI).
* Each entry echoes the `uri` and `version` the diagnostics were computed for,
  plus the `diagnostics` array (empty when the document is clean).
* Diagnostics within an entry MUST be sorted by `range.start` (line, then
  character).
* A document whose `languageId` is not served by the provider MUST produce an
  LPP error of kind `invalidLanguage` (the whole request fails).
* A document with a negative `version` MUST produce an LPP error of kind
  `invalidDocument`.

## 10. lpp/compile

Compile a document set into a single Workshop artifact. In LPP 1.1, an
entry-based request compiles the provider-loaded source closure as one unit;
the `compile.requiresSingleDocument` refusal applies only to a
document-supplied request that contains more than one document.

### 10.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "lpp/compile",
  "params": {
    "documents": { "file:///project/puzzle.xdl": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." } },
    "projectRoot": "file:///project"
  }
}
```

### 10.2 Result

```json
{
  "diagnostics": [
    { "uri": "file:///project/puzzle.xdl", "version": 3, "diagnostics": [] }
  ],
  "artifact": { "format": "x-demo/puzzle-eval-v1", "content": "{\"name\":\"clean\",\"ops\":[{\"arg\":2,\"name\":\"double\",\"op\":\"*\"},{\"arg\":1,\"name\":\"plus1\",\"op\":\"+\"}],\"solution\":[\"double\",\"double\"],\"start\":10,\"target\":40,\"value\":40}" }
}
```

* `diagnostics`: same shape as the `lpp/check` result.
* `artifact`: the compiled Workshop artifact, or `null`.
* The `artifact` MUST be `null` whenever any error-severity diagnostic is
  reported. The provider MAY return `null` artifact in other failure cases.
* The provider compiles the document set as a single unit. If the provider
  cannot compile the given set as one unit, it MUST refuse with a refusal
  whose `refusalCode` describes the requirement (for example
  `compile.requiresSingleDocument`).

**Artifact boundary.** The `WorkshopArtifact` is an opaque envelope. LPP
defines only the `format`/`content` shape; the payload semantics belong to the
format. The provider MUST NOT use the artifact to smuggle implementation
types: no provider AST/HIR, no Wright or workshop-rs internal IR, no Rust or
JSON-RPC-adjacent encoding is part of the artifact contract. Artifact content
is produced and consumed only by the provider (and, if a provider documents a
format for interoperation, by the ecosystem that owns that format).

**Artifact formats.** Format ids are strings; ids beginning with `lpp/` are
reserved for the protocol and MUST NOT be used by providers. Providers SHOULD
prefix format ids with a language or provider identifier (for example
`x-demo/puzzle-eval-v1`). A canonical Workshop artifact format (if any) is an
ecosystem decision owned outside this specification; LPP will not freeze one
without concrete evidence.

## 11. lpp/reconstruct

Reconstruct source text from a Workshop artifact. This is the inverse of
`lpp/compile` for the provider's own artifact formats.

### 11.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "lpp/reconstruct",
  "params": {
    "artifact": { "format": "x-demo/puzzle-eval-v1", "content": "{\"name\":\"clean\",...}" }
  }
}
```

### 11.2 Result

```json
{
  "source": "puzzle clean {\n  target = 40\n  ...\n}",
  "uri": "file:///project/puzzle.xdl"
}
```

* `source`: the reconstructed source text.
* `uri`: OPTIONAL suggested URI for the reconstructed source.

### 11.3 Failure behavior

* A well-formed artifact in a format the provider does not support MUST be
  answered with a refusal, `refusalCode` = `reconstruct.artifactFormatUnsupported`.
* A malformed artifact in a supported format (content does not parse, or does
  not match the format schema) MUST be answered with an LPP error of kind
  `invalidArtifact`.

## 12. lpp/symbols

List the symbols declared in a set of documents.

### 12.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "lpp/symbols",
  "params": {
    "documents": { "file:///project/puzzle.xdl": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." } }
  }
}
```

### 12.2 Result

```json
{
  "documents": [
    { "uri": "file:///project/puzzle.xdl", "version": 3, "symbols": [ { "name": "clean", "kind": "puzzle", "range": { "start": { "line": 0, "character": 7 }, "end": { "line": 0, "character": 12 } } } ] }
  ]
}
```

Symbols MUST be listed in declaration order within each document.

## 13. lpp/definition

Resolve the definition of the symbol at a position.

### 13.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "lpp/definition",
  "params": {
    "document": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." },
    "position": { "line": 7, "character": 16 }
  }
}
```

### 13.2 Result

```json
{ "locations": [ { "uri": "file:///project/puzzle.xdl", "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } } } ] }
```

* `locations`: the definition location(s) of the symbol at `position`. When
  the position is already on a declaration, the declaration itself is
  returned.
* When no symbol exists at `position`, the provider MUST refuse with
  `refusalCode` = `definition.noSymbolAtPosition`.
* A `position` outside the document MUST produce an LPP error of kind
  `invalidPosition`.

## 14. lpp/references

Find all references to the symbol at a position within the document.

### 14.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "lpp/references",
  "params": {
    "document": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." },
    "position": { "line": 4, "character": 6 },
    "includeDeclaration": true
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `position` | Position | Position of the symbol to search for. |
| `includeDeclaration` | boolean | Whether to include the declaration location in the result. |

### 14.2 Result

```json
{
  "locations": [
    { "uri": "file:///project/puzzle.xdl", "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } } },
    { "uri": "file:///project/puzzle.xdl", "range": { "start": { "line": 7, "character": 15 }, "end": { "line": 7, "character": 21 } } }
  ]
}
```

* Locations MUST be sorted by `range.start`. When `includeDeclaration` is
  true, the declaration location is listed first.
* When no symbol exists at `position`, the provider MUST refuse with
  `refusalCode` = `references.noSymbolAtPosition`.

## 15. lpp/rename

Compute source edits for a semantic rename of the symbol at a position. The
result is a set of source-oriented text edits: the client applies them to its
own document texts. LPP v1 defines no "rewrite the whole file from an AST"
mode; the provider MUST NOT return serialized ASTs or IR in place of edits.

### 15.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "lpp/rename",
  "params": {
    "documents": { "file:///project/puzzle.xdl": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." } },
    "positionDocumentUri": "file:///project/puzzle.xdl",
    "position": { "line": 4, "character": 6 },
    "newName": "twice"
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `positionDocumentUri` | string | The URI of the document (a key of `documents`) in which `position` is interpreted. REQUIRED. |
| `position` | Position | Position of the symbol to rename. |
| `newName` | string | The new name. The provider MUST validate it against the language's identifier rules. |

### 15.2 Result

```json
{
  "edits": [
    {
      "documentUri": "file:///project/puzzle.xdl",
      "version": 3,
      "textEdits": [
        { "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } }, "newText": "twice" },
        { "range": { "start": { "line": 7, "character": 15 }, "end": { "line": 7, "character": 21 } }, "newText": "twice" }
      ]
    }
  ]
}
```

* Each entry targets one document; `version` echoes the version of that
  document as received by the provider.
* Edits within one document MUST NOT overlap and MUST be sorted by
  `range.start`.
* The provider MUST NOT return edits for documents outside the request's
  `documents` set. If a correct rename would require editing a document the
  client did not send, the provider MUST refuse instead of producing a partial
  result.
* The provider MUST apply the rename consistently across all documents in the
  set (all references to the renamed symbol in received documents MUST be
  covered by the returned edits).
* Renames that would produce invalid source MUST be refused rather than
  returned: an invalid `newName` produces `refusalCode` =
  `rename.invalidName`; a collision with an existing symbol produces
  `refusalCode` = `rename.nameCollision`.

### 15.3 Refusals

* No symbol at `position`: `refusalCode` = `rename.noSymbolAtPosition`.
* `newName` invalid for the language: `refusalCode` = `rename.invalidName`.
* Rename would collide with an existing symbol: `refusalCode` = `rename.nameCollision`.
* Rename needs an unreceived document: `refusalCode` =
  `rename.requiresDocument` with `details` describing the requirement.

## 16. lpp/validateEdits

Validate a set of source edits against a document before the client applies
them. The provider applies the edits under the normative rules below and
checks the result.

### 16.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "lpp/validateEdits",
  "params": {
    "document": { "uri": "file:///project/puzzle.xdl", "languageId": "x-demo-lang", "version": 3, "text": "..." },
    "edits": [
      { "range": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } }, "newText": "twice" }
    ]
  }
}
```

### 16.2 Result

```json
{ "valid": true, "version": 3 }
```

Invalid result:

```json
{ "valid": false, "version": 3, "reason": "overlappingEdits", "failingEditIndex": 1 }
```

| Field | Type | Description |
| --- | --- | --- |
| `valid` | boolean | Whether the edit set applies cleanly and the result is well-formed. |
| `version` | integer | The version of the document the edits were validated against. |
| `reason` | string, OPTIONAL | Present iff `valid` is false. One of: `overlappingEdits`, `rangeOutOfBounds`, `syntaxError`. |
| `failingEditIndex` | integer, OPTIONAL | Index (in the request's `edits` array, as received) of the offending edit. Present for `overlappingEdits` and `rangeOutOfBounds`; absent for `syntaxError`. |

### 16.3 Normative edit application rules

The provider MUST apply edits as follows:

1. Validate every edit's range against the original document text (bounds,
   `start <= end`). Any violation produces `rangeOutOfBounds` with the index
   of the first offending edit.
2. Sort edits by `range.start` (line, then character).
3. After sorting, if any edit's `range.start` is before the previous edit's
   `range.end` (comparing in original coordinates), the set is invalid:
   `overlappingEdits` with the index of the later edit in the **original**
   request order.
4. Apply the sorted edits in order to produce the resulting text.
5. Parse the resulting text. Any syntax or semantic error produces
   `syntaxError`.

## 17. lpp/shutdown

Graceful termination request.

### 17.1 Request

```json
{ "jsonrpc": "2.0", "id": 10, "method": "lpp/shutdown", "params": {} }
```

### 17.2 Response

```json
{ "jsonrpc": "2.0", "id": 10, "result": null }
```

After sending the response the provider MUST exit with status 0. The client
SHOULD NOT rely on receiving `lpp/shutdown` responses indefinitely; it MAY
terminate the process at any time.

## 18. Errors and refusals

### 18.1 Error model

LPP distinguishes three outcome classes:

1. **Success**: a `result` in the response.
2. **Error**: the request could not be fulfilled and the client should treat
   it as a failure of the request or of the session. Transport-level failures
   use the standard JSON-RPC codes ([Section 4.1](#41-standard-json-rpc-errors)).
3. **Refusal**: the provider understood the request and deliberately declines
   it for a documented, machine-readable reason. Refusals are normal outcomes
   (a rename of a non-symbol, an unsupported artifact format) that the client
   should surface without treating the session as broken.

### 18.2 LPP error shape

All LPP-defined errors use JSON-RPC error code `-32000` and carry a structured
`data.lpp` object:

```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "error": {
    "code": -32000,
    "message": "rename refused: no symbol at position",
    "data": {
      "lpp": {
        "kind": "refusal",
        "details": {
          "refusalCode": "rename.noSymbolAtPosition",
          "uri": "file:///project/puzzle.xdl",
          "range": { "start": { "line": 1, "character": 2 }, "end": { "line": 1, "character": 3 } }
        }
      }
    }
  }
}
```

### 18.3 LPP error kinds

| `data.lpp.kind` | `details` | When |
| --- | --- | --- |
| `protocolVersionMismatch` | `{ "supportedProtocolVersions": [string] }` | `lpp/initialize` with an unsupported protocol version. |
| `invalidRequest` | `{ "reason": string }` | Session violations: `notInitialized`, `alreadyInitialized`, `notificationNotSupported`. |
| `invalidLanguage` | `{ "languageId": string }` | A document's `languageId` is not served by the provider. |
| `invalidDocument` | `{ "uri"?: string, "reason": string }` | A document is unusable (for example a negative version). |
| `invalidEntry` | `{ "entryUri": string, "reason": string }` | A project entry has an unsupported URI or language. LPP 1.1 only. |
| `projectLoadFailed` | `{ "entryUri": string, "reason": string, "uri"?: string }` | A filesystem-backed project entry or required source file could not be loaded. LPP 1.1 only. |
| `invalidPosition` | `{ "uri": string, "position": Position }` | A position outside the document. |
| `invalidArtifact` | `{ "reason": string }` | An artifact in a supported format whose content is malformed. |
| `capabilityUnavailable` | `{ "capability": string, "method": string }` | A method was invoked whose capability was not negotiated. |
| `refusal` | `{ "refusalCode": string, "uri"?: string, "range"?: Range }` | A deliberate decline; `refusalCode` identifies the reason. |

`data.lpp` MUST NOT be attached to the standard JSON-RPC errors
([Section 4.1](#41-standard-json-rpc-errors)).

### 18.4 Refusal codes

`refusalCode` values are provider-defined strings. The codes used by the
reference mock provider are documented as examples in
[Appendix B](#appendix-b-reference-refusal-codes-non-normative). Clients MUST
treat unknown refusal codes as opaque strings: display the `message`, never
branch on the unknown code.

The `message` field MUST be human-readable. `details` is machine-readable;
clients MUST NOT parse `message`.

## 19. Protocol evolution and version negotiation

### 19.1 Versioning scheme

* Protocol versions are strings of the form `MAJOR.MINOR` (for example
  `"1.0"`). LPP 1.0 is the first published version and LPP 1.1 is an additive
  revision of the same protocol major version.
* `MAJOR` changes are breaking: message shapes, method semantics, or framing
  may change. A breaking change always produces a new MAJOR version, and
  clients and providers speaking different MAJOR versions are never expected
  to interoperate.
* `MINOR` changes are additive: new OPTIONAL request/result fields, new
  OPTIONAL methods, or new capability ids that providers may choose not to
  implement. A provider MUST ignore unknown fields it does not understand, and
  a client MUST NOT depend on fields the provider did not advertise via
  capabilities.

### 19.2 Negotiation

* The client sends the version it wants in `lpp/initialize`.
* The provider either accepts it (echoing the version in the result) or fails
  with `protocolVersionMismatch` listing `supportedProtocolVersions`.
* A client that receives the mismatch MUST pick the highest mutually supported
  version and restart the session, or terminate. LPP 1.1 clients MAY use the
  `projectLoading` capability; clients that need it MUST request `"1.1"`.
* A provider MUST support at least one of the versions it lists in
  `supportedProtocolVersions`.

### 19.3 Rules for introducing changes

1. Every protocol-visible change MUST be accompanied by a normative spec
   update and matching conformance fixtures in `conformance/fixtures/v<major>/`.
2. New capabilities MUST be advertised via the capability negotiation contract
   and MUST be independently optional.
3. Wire compatibility claims are grounded in the conformance suite, not in
   implementation identity.
4. No new protocol version is finalized before Wright or any first-party
   provider relies on it in production. The conformance suite must pass
   against the reference mock provider with no schema changes to the fixtures
   of prior versions.

## 20. Conformance

The conformance suite in `conformance/` is the normative evidence for the wire
contract:

* `conformance/fixtures/v1/`: versioned JSON-RPC message fixtures covering
  initialization, capability negotiation, diagnostics, check, compile,
  reconstruct, symbols, definition, references, rename, edit validation,
  project loading, errors/refusals, protocol mismatch, malformed messages,
  and shutdown. The same directory covers LPP 1.0 and its LPP 1.1 additive
  revision.
* `conformance/runner/`: a runner that replays fixtures against any provider
  binary and compares responses exactly.
* `conformance/mock-provider/`: the reference provider for the demonstration
  language `x-demo-lang` (a puzzle/equation DSL). The mock is
  spawnable as a stdio binary so client-side integration can use it
  end-to-end.

Running the suite and interpreting results is documented in
`conformance/README.md`. Conformance proves wire-contract conformance; it does
not prove Workshop semantic correctness, game runtime behavior, or
performance.

## Appendix A: Message and type index

Methods:

| Method | Capability | Params | Result |
| --- | --- | --- | --- |
| `lpp/initialize` | none | `{ protocolVersion, clientInfo? }` | `{ protocolVersion, serverInfo, languages, capabilities }` |
| `lpp/shutdown` | none | `{}` | `null` |
| `lpp/check` | `check`; plus `projectLoading` for an LPP 1.1 `entry` request | `{ documents, projectRoot? }` or `{ entry, projectRoot? }` | `{ documents: [{ uri, version, diagnostics }] }` |
| `lpp/compile` | `compile`; plus `projectLoading` for an LPP 1.1 `entry` request | `{ documents, projectRoot? }` or `{ entry, projectRoot? }` | `{ diagnostics: [{ uri, version, diagnostics }], artifact }` |
| `lpp/reconstruct` | `reconstruct` | `{ artifact }` | `{ source, uri? }` |
| `lpp/symbols` | `symbols` | `{ documents, projectRoot? }` | `{ documents: [{ uri, version, symbols }] }` |
| `lpp/definition` | `definition` | `{ document, position }` | `{ locations }` |
| `lpp/references` | `references` | `{ document, position, includeDeclaration }` | `{ locations }` |
| `lpp/rename` | `rename` | `{ documents, positionDocumentUri, position, newName, projectRoot? }` | `{ edits: [{ documentUri, version, textEdits }] }` |
| `lpp/validateEdits` | `editValidation` | `{ document, edits }` | `{ valid, version, reason?, failingEditIndex? }` |

Types: `Position`, `Range`, `TextEdit`, `Document`, `DocumentSet`, `ProjectEntry`,
`Diagnostic`, `Location`, `Symbol`, `WorkshopArtifact` (see
[Section 6](#6-common-data-types)).

## Appendix B: Reference refusal codes (non-normative)

The reference mock provider uses these refusal codes. They are examples of
provider-defined codes; other providers MAY use different codes.

| `refusalCode` | Meaning |
| --- | --- |
| `compile.requiresSingleDocument` | Compile requires exactly one document. |
| `reconstruct.artifactFormatUnsupported` | The artifact format is not supported. |
| `definition.noSymbolAtPosition` | No symbol at the given position. |
| `references.noSymbolAtPosition` | No symbol at the given position. |
| `rename.noSymbolAtPosition` | No symbol at the given position. |
| `rename.invalidName` | The new name is not a valid identifier. |
| `rename.nameCollision` | The new name collides with an existing symbol. |

## Appendix C: Example session transcript

Complete session against the reference mock provider (newlines between
messages elided for readability):

```text
--> {"jsonrpc":"2.0","id":1,"method":"lpp/initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"wright","version":"0.2.0"}}}
<-- {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"1.0","serverInfo":{"name":"lpp-mock-provider","version":"0.1.0"},"languages":[{"id":"x-demo-lang","extensions":["xdl"]}],"capabilities":{"check":true,"compile":true,"reconstruct":true,"symbols":true,"definition":true,"references":true,"rename":true,"editValidation":true}}}

--> {"jsonrpc":"2.0","id":2,"method":"lpp/check","params":{"documents":{"file:///project/puzzle.xdl":{"uri":"file:///project/puzzle.xdl","languageId":"x-demo-lang","version":3,"text":"puzzle clean {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double ]\n}"}}}}
<-- {"jsonrpc":"2.0","id":2,"result":{"documents":[{"uri":"file:///project/puzzle.xdl","version":3,"diagnostics":[]}]}}

--> {"jsonrpc":"2.0","id":3,"method":"lpp/rename","params":{"documents":{"file:///project/puzzle.xdl":{"uri":"file:///project/puzzle.xdl","languageId":"x-demo-lang","version":3,"text":"puzzle clean {\n  target = 40\n  start = 10\n  ops {\n    double: x => x * 2\n    plus1: x => x + 1\n  }\n  solution = [ double, double ]\n}"}},"positionDocumentUri":"file:///project/puzzle.xdl","position":{"line":4,"character":6},"newName":"twice"}}
<-- {"jsonrpc":"2.0","id":3,"result":{"edits":[{"documentUri":"file:///project/puzzle.xdl","version":3,"textEdits":[{"range":{"start":{"line":4,"character":4},"end":{"line":4,"character":10}},"newText":"twice"},{"range":{"start":{"line":7,"character":15},"end":{"line":7,"character":21}},"newText":"twice"},{"range":{"start":{"line":7,"character":23},"end":{"line":7,"character":29}},"newText":"twice"}]}]}}

--> {"jsonrpc":"2.0","id":4,"method":"lpp/shutdown","params":{}}
<-- {"jsonrpc":"2.0","id":4,"result":null}
(provider exits with status 0)
```
