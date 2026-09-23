# LPP v1 — Analysis and compilation

[← LPP v1 specification index](README.md)

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

Compile a document set into a single Workshop artifact. In LPP 1.1, 1.2, and
1.3, an entry-based request compiles the provider-loaded source closure as one
unit;
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
* `sourceIdentity`: defined only in LPP 1.3. In an LPP 1.3 session, the
  provider MUST advertise the `sourceIdentity` capability. If it advertises
  `sourceIdentity: true`, an entry-based compile result MUST include a
  lower-case SHA-256 hex digest of the provider-selected primary source text.
  The provider owns effective entry selection. If the capability is false, the
  field MUST be omitted. Document-supplied requests MAY omit it. LPP 1.1 and
  1.2 compile results MUST NOT include this field.
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

