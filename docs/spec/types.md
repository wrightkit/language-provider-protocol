# LPP v1 — Common data types

[← LPP v1 specification index](README.md)

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
[Section 8.1](#81-entry-based-project-requests-lpp-11) and extended to
directory targets in [Section 8.2](#82-directory-project-requests-lpp-12).

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

A project entry identifies the source target selected by the client for a
provider-owned filesystem project load. The example below selects a file:

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
* `kind`: OPTIONAL for LPP 1.1 through 1.4. When omitted, it requests the
  existing file-entry behavior. LPP 1.2 and later clients MUST use `"directory"` when
  the provider must discover the effective project entry from a directory;
  `"file"` may be used explicitly for file-entry behavior.

The entry identifies the user's selected source target only. The provider
determines the effective project root and source closure according to the
source language's rules; the client MUST NOT infer or supply that closure.

