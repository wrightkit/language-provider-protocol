# LPP v1 — Conformance and reference material

[← LPP v1 specification index](README.md)

## 20. Conformance

The conformance suite in `conformance/` is the normative evidence for the wire
contract:

* `conformance/fixtures/v1/`: versioned JSON-RPC message fixtures covering
  initialization, capability negotiation, diagnostics, check, compile,
  reconstruct, symbols, definition, references, rename, edit validation,
  project loading, errors/refusals, protocol mismatch, malformed messages,
  and shutdown. The same directory covers LPP 1.0, its LPP 1.1 file-entry
  revision, its LPP 1.2 directory-target revision, its LPP 1.3
  source-identity revision, and its LPP 1.4 artifact-format negotiation
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
| `lpp/check` | `check`; plus `projectLoading` for an LPP 1.1+ `entry` request | `{ documents, projectRoot? }` or `{ entry, projectRoot? }` | `{ documents: [{ uri, version, diagnostics }] }` |
| `lpp/compile` | `compile`; plus `projectLoading` for an LPP 1.1+ `entry` request and `sourceIdentity` for an LPP 1.3+ entry result | `{ documents, projectRoot?, acceptedArtifactFormats? }` or `{ entry, projectRoot?, acceptedArtifactFormats? }` (`acceptedArtifactFormats`: LPP 1.4) | `{ diagnostics: [{ uri, version, diagnostics }], sourceIdentity?, artifact }` |
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
| `compile.artifactFormatUnsupported` | None of the accepted artifact formats is supported. |
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

