# ADR 0004: Artifact Format Negotiation for `lpp/compile`

- Status: Accepted
- Decision date: 2026-09-24 (UTC)
- Scope: LPP 1.4 `lpp/compile` requests

## Context

`lpp/compile` returns an opaque `format`/`content` artifact in a
provider-chosen format. The source-mapping decision in
[workshop-rs#271](https://github.com/wrightkit/workshop-rs/issues/271)
(workshop-rs ADR-0013) expects providers to be able to return a richer
Workshop artifact format, for example `workshop-rs/mapped-text-v1`, when the
client can consume it. LPP must let a client say what it accepts without
defining or validating any Workshop format.

## Decision

LPP 1.4 adds an optional `acceptedArtifactFormats` field to `lpp/compile`
requests: a non-empty, ordered array of format ids.

- The provider returns the first listed format it can produce. Unknown ids
  are skipped.
- If none can be produced and an artifact would otherwise be returned, the
  provider refuses (`compile.artifactFormatUnsupported` in the reference
  provider) instead of falling back to an unlisted format. A client that wants
  the provider default lists it explicitly.
- Absent field: behavior is identical to LPP 1.3.
- The field is valid only in sessions that negotiated `"1.4"`; earlier
  sessions reject it with `-32602`. A 1.4 provider MUST implement it, so no
  capability id is added.

The artifact stays an opaque envelope; format payload semantics remain owned
outside LPP.

## Consequences

- Clients get a deterministic result format and never receive a payload they
  did not list.
- Providers that only have one format can serve 1.4 by matching that id or
  refusing.
- A client wanting graceful fallback pays one extra list entry.

## Alternatives considered

### Silent fallback to the provider default

Rejected because the client could receive a format it cannot consume and would
have to detect the mismatch after the fact.

### A separate capability id per format or for negotiation

Rejected as speculative: the version gate already lets clients require the
feature, and format ids are opaque strings.

### Ignore the field in pre-1.4 sessions

Rejected because a client could not tell whether negotiation took effect.
