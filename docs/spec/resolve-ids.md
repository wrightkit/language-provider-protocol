# LPP v1 — Canonical id resolution

[← LPP v1 specification index](README.md)

## 22. lpp/resolveIds (LPP 1.6)

Map a set of canonical Workshop ids to the provider's language spellings:
the reverse direction of `lpp/lookup`, which resolves free text and issues
opaque identities. A client that holds canonical ids, for example the ids in a
rule's content, uses `lpp/resolveIds` to learn how the language writes each
one. The provider owns the spellings; the client MUST NOT derive or guess
them.

The canonical ids are owned by the Workshop ecosystem, not by LPP. LPP treats
each id as an opaque string and defines no id vocabulary.

`lpp/resolveIds` is available only in a session that negotiated protocol
version `"1.6"` or later and whose provider advertised
`capabilities.resolveIds: true`. A request in any other session MUST be
answered with an LPP error of kind `capabilityUnavailable`, including a session
that negotiated an earlier version (see
[Section 7.3](project-loading.md#73-capability-negotiation)).

### 22.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "lpp/resolveIds",
  "params": {
    "languageId": "x-demo-lang",
    "calls": ["applyImpulse", "setStatusEffect"],
    "enums": { "Status": ["STUNNED"] }
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `languageId` | string | REQUIRED. The language to resolve for. It MUST be a language id advertised in `lpp/initialize`; a language the provider does not serve MUST produce an LPP error of kind `invalidLanguage`. |
| `calls` | array of string | OPTIONAL. Canonical ids of actions, values, and operators. Absent is equivalent to `[]`. |
| `enums` | object | OPTIONAL. Enum members grouped by canonical domain id: each key is a domain id and each value is an array of canonical member ids. Absent is equivalent to `{}`. |

* The total number of ids, `calls` plus every `enums` member array, counting
  duplicates, MUST NOT exceed **1024**. A request over the bound MUST be
  rejected with JSON-RPC `-32602`; a provider MUST NOT truncate. A client with
  more ids sends several requests.
* A `params` value that does not match this schema MUST be rejected with
  `-32602`, including a missing `languageId` or a wrong element or container
  type.
* A request carries no `documents` or `entry`: the provider MUST answer from
  its language vocabulary alone and MUST NOT require a loaded project.

### 22.2 Result

```json
{
  "calls": { "applyImpulse": "shove", "setStatusEffect": "afflict" },
  "enums": { "Status": { "STUNNED": "stunned" } }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `calls` | object | REQUIRED. Maps each requested call id the language spells to its spelling. |
| `enums` | object | REQUIRED. Maps a domain id to an object that maps each requested member id the language spells to its spelling. |

* A spelling is the bare text the client would write in the language, as in
  `lpp/lookup`; it is never a rendered signature. Its format is
  provider-defined.
* **Omission:** an id the language has no spelling for, whether it is unknown
  to the provider or exists only after lowering, MUST be omitted. A provider
  MUST NOT report it as an error, return a placeholder, or guess a spelling.
  A domain none of whose requested members is spelled MUST be omitted. A
  client treats an absent key as "no spelling".
* The result contains only requested ids, so it is bounded by the request.
* Results MUST be deterministic: the same request against the same provider
  version MUST return the same result. Key order carries no meaning.
* A request with no ids produces `{ "calls": {}, "enums": {} }`.

### 22.3 Failure behavior

* Capability not negotiated, including a session that negotiated a version
  earlier than 1.6: LPP error of kind `capabilityUnavailable`.
* `languageId` not served by the provider: LPP error of kind `invalidLanguage`.
* Invalid `params`, or more than 1024 ids: JSON-RPC `-32602`.
