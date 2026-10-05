# LPP v1 — Name lookup

[← LPP v1 specification index](README.md)

## 21. lpp/lookup (LPP 1.5)

Resolve a free-text name guess to the provider's spellings and signature facts
for one of its languages — callables, enum domains and members, settings keys,
and similar catalog entries — without requiring a loaded project. The provider
owns the name vocabulary and returns facts; it does not render a signature
string and does not apply a client presentation budget. Presentation (for
example rendering a signature or truncating a member list for display) is the
client's concern.

`lpp/lookup` is available only in a session that negotiated protocol version
`"1.5"` or later and whose provider advertised `capabilities.lookup: true`.
An `lpp/lookup` request in any other session MUST be answered with an LPP
error of kind `capabilityUnavailable` — including a session that negotiated an
earlier version, where the capability is absent (see
[Section 7.3](#73-capability-negotiation)).

### 21.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "lpp/lookup",
  "params": {
    "languageId": "x-demo-lang",
    "query": "multiply",
    "kind": "operator",
    "within": { "kind": "enum", "value": "x-demo:enum/operator" },
    "locale": "en-us",
    "limit": 3
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `languageId` | string | REQUIRED. The language to query. It MUST be a language id advertised by the provider in `lpp/initialize`; a language the provider does not serve MUST produce an LPP error of kind `invalidLanguage`. |
| `query` | string | OPTIONAL. Free text to resolve: a display name, a near spelling, or a guess. Absent or empty applies no text constraint and lists the in-scope entries. |
| `kind` | string | OPTIONAL. Restricts the result to one provider-defined entry `kind`. A kind the provider does not use matches no entries. |
| `within` | object | OPTIONAL. Restricts the result to the children of one named scope; see [Section 21.3](#213-within-selector). |
| `locale` | string | OPTIONAL. The locale the provider SHOULD use for `displayName`. When it cannot serve the requested locale it MAY fall back to its default; the choice MUST be deterministic. |
| `limit` | integer | OPTIONAL. The maximum number of entries to return; MUST be an integer `>= 1`. When absent, the provider applies its own bound; the result is always bounded. |

A `params` value that does not match this schema MUST be rejected with
JSON-RPC `-32602`. A request carries no `documents` or `entry`: the provider
MUST answer from its language vocabulary alone and MUST NOT require a loaded
project or any source document.

### 21.2 Result

```json
{
  "entries": [
    {
      "identity": "x-demo:operator/multiply",
      "kind": "operator",
      "spelling": "*",
      "displayName": "multiply",
      "callable": {
        "receiver": "integer",
        "parameters": [
          { "name": "arg", "type": "integer", "required": true }
        ]
      }
    }
  ]
}
```

| Field | Type | Description |
| --- | --- | --- |
| `identity` | string | REQUIRED. A provider-issued opaque identity. Clients MUST NOT parse it. The provider MUST accept identities it issued as `within` values for the matching selector kinds while they remain valid for the same provider version. |
| `kind` | string | REQUIRED. A provider-defined entry kind (for example `action`, `value`, `parameter`, `enumMember`, `setting`). LPP does not define a fixed kind vocabulary. |
| `spelling` | string | REQUIRED. The bare spelling in the requested language: the text the client would write. It is never a rendered signature. |
| `displayName` | string | REQUIRED. The display name for the requested or fallback locale. |
| `callable` | object | OPTIONAL. Callable facts; see [Section 21.4](#214-callable-facts). Present when the entry is invocable. |
| `enum` | object | OPTIONAL. An enum domain the entry defines or a parameter/setting accepts: `{ "domain": string, "members": [string] }`. `domain` is the domain entry's `identity` and is a valid `within` value for selector kind `"enum"`; `members` is the complete member spelling list of that domain. |
| `parameter` | object | OPTIONAL. On entries that describe a callable's parameter, the parameter facts `{ "type": string, "required": boolean, "default"?: scalar, "enum"?: enum domain }`; the parameter's `name` is the entry `spelling`. |
| `setting` | object | OPTIONAL. On entries that describe a settings key, the value form the setting accepts: `{ "type": string, "minimum"?: number, "maximum"?: number, "enum"?: enum domain }`. `type` is a provider-defined value-domain name. |

* `entries` is the ranked result. Ranking and match thresholds are
  provider-defined and are not part of the wire contract.
* Results MUST be deterministic: the same request against the same provider
  version MUST return the same entries in the same order. Entries the
  provider considers tied MUST be ordered by a stable provider-defined rule.
* A query that matches no entry produces `"entries": []`; an empty result is
  not a refusal.
* A request with no `within` and an absent or empty `query` lists the
  provider's entries in its stable order, bounded by `limit` like any other
  result.

### 21.3 `within` selector

```json
{ "kind": "callable", "value": "x-demo:operator/multiply" }
```

`within` names one scope whose children the result lists. `kind` is REQUIRED
and MUST be one of `"callable"`, `"enum"`, or `"settings"`; any other value
MUST be rejected with `-32602`. `value` is a REQUIRED string.

| `within.kind` | `value` | Entries returned |
| --- | --- | --- |
| `callable` | The `identity` of an entry with `callable` facts. | One entry per parameter of that callable, in call/declaration order. Each entry carries `parameter` facts; `kind` is provider-defined (for example `parameter`). |
| `enum` | The `identity` of an entry that defines an enum domain. | The member spellings of that domain as entries, in the domain's order. |
| `settings` | A settings path prefix, matched segment by segment against the provider's settings paths. | The immediate children under the prefix: the settings and intermediate path segments directly below it, each with `spelling` equal to its own path segment. An empty `value` lists the root. |

* `within` selects the scope; `query` then filters or ranks its children by
  the same rules as an unscoped request. When `query` is absent or empty, the
  result lists the scope's children in the provider's stable order (parameter
  entries MUST keep call order).
* A `within` selector that names no scope the provider knows — an unknown
  `callable` or `enum` identity, or a settings prefix that is not a proper
  segment-wise prefix of any settings path — MUST be refused with a refusal
  whose `refusalCode` describes the failure (for example
  `lookup.unknownWithin`). A selector that names an existing but empty scope
  produces `"entries": []` instead.

### 21.4 Callable facts

```json
"callable": {
  "receiver": "integer",
  "parameters": [
    { "name": "arg", "type": "integer", "required": true },
    { "name": "mode", "type": "RoundingMode", "required": false,
      "default": "Up", "enum": { "domain": "opy:enum/RoundingMode", "members": ["Up", "Down"] } }
  ]
}
```

* `parameters`: REQUIRED and ordered. Each parameter carries:
  * `name`: REQUIRED string.
  * `type`: REQUIRED provider-defined type name.
  * `required`: REQUIRED boolean.
  * `default`: OPTIONAL JSON scalar (string, number, boolean, or `null`) — the
    parameter's default value in the provider's terms.
  * `enum`: OPTIONAL enum domain the parameter accepts, as defined in
    [Section 21.2](#212-result).
* `receiver`: OPTIONAL provider-defined type name of the value the callable is
  invoked on, for languages whose callables take a receiver.

The provider supplies facts only. It MUST NOT render a signature string or
apply a client's token-budget rules; clients build any rendered presentation
from these fields.

### 21.5 Failure behavior

* Capability not negotiated (including a session that negotiated a version
  earlier than 1.5): LPP error of kind `capabilityUnavailable`.
* `languageId` not served by the provider: LPP error of kind
  `invalidLanguage`.
* Invalid `params` (missing required fields, wrong types, `limit` outside its
  bound, a `within.kind` outside the closed set): JSON-RPC `-32602`.
* A `within` selector naming no known scope: refusal; the reference mock
  provider uses `lookup.unknownWithin` (see
  [Appendix B](#appendix-b-reference-refusal-codes-non-normative)).
