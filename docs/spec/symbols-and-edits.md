# LPP v1 — Symbols and source edits

[← LPP v1 specification index](README.md)

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

