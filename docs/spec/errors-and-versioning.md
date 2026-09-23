# LPP v1 — Errors and versioning

[← LPP v1 specification index](README.md)

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
| `invalidEntry` | `{ "entryUri": string, "reason": string }` | A project entry/target has an unsupported URI, language, or kind. LPP 1.1+. |
| `projectLoadFailed` | `{ "entryUri": string, "reason": string, "uri"?: string }` | A filesystem-backed project entry/target or required source file could not be loaded. LPP 1.1+. |
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
  `"1.0"`). LPP 1.0 is the first published version; LPP 1.1 adds file-entry
  project loading, LPP 1.2 adds directory targets, and LPP 1.3 adds the
  optional `sourceIdentity` capability for entry-based compile results.
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
  version and restart the session, or terminate. LPP 1.1, 1.2, and 1.3 clients
  MAY use the `projectLoading` capability; clients that need directory targets
  MUST request `"1.2"` or `"1.3"`. Clients that need source identity MUST
  request `"1.3"` and require `sourceIdentity: true` in the result capabilities.
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

