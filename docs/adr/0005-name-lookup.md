# ADR 0005: Name Lookup via `lpp/lookup`

- Status: Accepted
- Decision date: 2026-10-05 (UTC)
- Scope: LPP 1.5 `lpp/lookup` requests

## Context

Coding agents working through LPP can only discover a language's names by
guess-and-check: submit source, read unknown-name diagnostics, guess again.
When that fails they leave the protocol entirely — mining binaries, reading
installed files, or fetching upstream documentation — for answers that are
owner facts the provider already holds (spellings, parameter names, enum
members, settings keys). Roughly 58% of failing OverPy calls in the Wright
benchmark involved unknown-name diagnostics, which motivated the Wright
domain-intelligence contract in wright ADR-0017 and the accepted
name/signature lookup decision in wright ADR-0021
(`wrightkit/wright#482`). This is the concrete provider-backed consumer that
earlier drafts of that ADR said should exist before extending LPP
([issue #45](https://github.com/wrightkit/language-provider-protocol/issues/45)).

## Decision

LPP 1.5 adds `lpp/lookup` behind the optional `lookup` capability. The method
resolves a free-text name guess against the provider's language vocabulary —
callables, enum domains and members, settings keys, and similar catalog
entries — without a loaded project.

- The request carries `languageId`, optional free-text `query`, optional
  `kind`, an optional `within` selector (`callable` parameters, an `enum`
  domain's members, or a `settings` path prefix's children), optional
  `locale`, and optional `limit`.
- Each ranked entry carries the provider-issued `identity`, a provider-defined
  `kind`, the bare `spelling`, a `displayName`, plus `callable` facts
  (ordered parameters, optional `receiver`), `enum` domain members,
  `parameter` facts, or `setting` value forms where they apply.
- Results are deterministic and always bounded; entries the provider
  considers tied keep a stable provider-defined order.
- A request in a session that did not negotiate the capability — including a
  session that negotiated an earlier version — is answered with
  `capabilityUnavailable`.
- The provider returns facts only: it does not render a signature string and
  does not apply a client presentation budget. Ranking policy, kind
  vocabulary, and the language names themselves remain provider-owned.

## Consequences

- Version `"1.5"` is added; providers that support it advertise `lookup` only
  when they fully implement the method, and pre-1.5 sessions are unaffected.
- `lookup.unknownWithin` is registered as a reference refusal code.
- Conformance fixtures cover ranking determinism, bounded results, `within`
  scopes, and the absent capability; the reference mock provider implements
  the method for `x-demo-lang`.
- Language owners (Workshop, OPY) implement the method in their providers;
  Wright consumes it for name/signature queries (`wrightkit/wright#529`).

## Alternatives considered

### Diagnostics-only discovery

Keep agents guessing names in source and reading unknown-name diagnostics.
Rejected: it is the measured failure mode; nearest-candidate hints help a
failed call but do not give a positive query.

### Client-side extraction

Mine binaries, installed files, or upstream documentation outside LPP.
Rejected: proven wasteful and wrong; the answers are owner facts the provider
already holds.

### Render a signature string server-side

Simpler wire shape, but rejected because it bakes one client's presentation —
signature format, budget, member truncation — into the protocol. Facts travel
better across clients.

### Require a loaded project

Rejected: the queries this serves — "what is this called, and what does it
take, in this language" — are project-independent.
