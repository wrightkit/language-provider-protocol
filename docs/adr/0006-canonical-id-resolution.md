# ADR 0006: Canonical Id Resolution via `lpp/resolveIds`

- Status: Accepted
- Decision date: 2026-10-11 (UTC)
- Scope: LPP 1.6 `lpp/resolveIds` requests

## Context

Wright is adding structured rule content that names actions, values,
operators, and enum members by canonical Workshop id, such as `applyImpulse`
and `Status`/`STUNNED` (`wrightkit/wright#615`, ADR-0024 in that repository).
For an OverPy project the result must also carry the OverPy spelling of each
id that occurs in the content. Wright must not invent spellings or translate
names itself, so the provider has to supply them.

`lpp/lookup` (LPP 1.5, ADR 0005) cannot do this. It resolves free text, and
the `identity` it issues is opaque to the client, so there is no path from a
canonical id to a spelling.

## Decision

LPP 1.6 adds `lpp/resolveIds` behind the optional `resolveIds` capability.

- The request carries `languageId`, optional `calls` (an array of canonical
  ids for actions, values, and operators), and optional `enums` (enum
  members grouped by canonical domain id).
- The result has the same shape: `calls` maps id to spelling, and `enums`
  maps domain to member to spelling. It contains only requested ids that the
  language spells.
- An id with no spelling is omitted: no error, no placeholder, no guess. A
  domain with no spelled member is omitted.
- A request holds at most 1024 ids in total, duplicates counted. A larger
  request is rejected with `-32602` and is never truncated.
- A request needs no loaded project, as with `lpp/lookup`.
- LPP treats canonical ids as opaque strings. The Workshop ecosystem owns
  the vocabulary; LPP defines none.
- A session that did not negotiate the capability, including one on an
  earlier version, is answered with `capabilityUnavailable`.

## Consequences

- Version `"1.6"` is added as an additive minor version. Sessions on 1.5 and
  earlier are unaffected.
- A provider advertises `resolveIds` only when it fully implements the
  method. Wright reports the names as unavailable when it is absent.
- An id the provider does not know is treated as unmapped, so a newer client
  vocabulary does not fail against an older provider.
- Conformance fixtures cover mapping, omission, the bound, the version gate,
  the absent capability, an invalid language, and invalid params. The mock
  provider maps a small fixed table for `x-demo-lang` with spellings unlike
  the ids.
- `opy-rs` implements the capability from its own mapping from Workshop names
  to OverPy spellings (`wrightkit/opy-rs#515`).

## Alternatives considered

### A canonical-id selector on `lpp/lookup`

Rejected. `lookup` is a ranked, limited free-text search with opaque
identities. Adding an exact-id mode would give one method two semantics and
need rules for combining the modes.

### A flat array of `{kind, id}` entries

Rejected. It leaves room for id categories nobody has asked for. A new
category can be added later as an optional field under the minor-version
rules.

### A provider-declared maximum

Rejected. Callers send only the ids occurring in a bounded result, so a fixed
bound serves them, and a negotiated maximum adds a field with no consumer.

### Reporting unmapped ids as an error or an `unmapped` list

Rejected. A missing key already says "no spelling", and a guessed or
placeholder spelling would break the rule that spellings are not invented.

### Truncating an oversized request

Rejected. A client would read a missing key as "no spelling" when the
provider had only not looked.
