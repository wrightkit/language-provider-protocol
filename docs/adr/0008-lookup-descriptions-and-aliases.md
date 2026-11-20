# ADR 0008: Lookup Descriptions and Aliases

- Status: Accepted
- Decision date: 2026-11-20 (UTC)
- Scope: LPP 1.8 `lpp/lookup` entries

## Context

An `lpp/lookup` entry ([ADR 0005](0005-name-lookup.md)) gives a client the
spelling, kind, identity and callable facts of a construct. It does not say
what the construct does, and it does not say which other spellings the
language accepts for it. The Workshop owner already holds descriptions for
actions and values, so a client can answer "what is this" for Workshop and
cannot for an OverPy provider. OverPy also accepts alternative spellings for
some callables (the `opy-rs` manifest declares 20, each with evidence), and a
client that receives the canonical spelling for an alias query cannot tell the
user why
([Issue #113](https://github.com/wrightkit/language-provider-protocol/issues/113)).

## Decision

LPP 1.8 adds two OPTIONAL result fields to `lpp/lookup` entries. It adds no
capability id.

- `description`: plain text, no markup, in the requested `locale`, with the
  same deterministic fallback as `displayName`. It appears on an entry and on
  each `callable.parameters[]` item. A provider with no description omits it.
- `aliases`: on an entry, the other spellings the language accepts as source for
  the same construct. It is non-empty when present and never contains
  `spelling`, which stays the canonical spelling. The provider lists only
  spellings the language accepts.

A provider MUST NOT send either field in a session that negotiated a version
before 1.8, and a client MUST NOT depend on either in such a session. In a 1.8
session an absent field means "not provided".

## Consequences

- Fixtures 71 and 72 cover a 1.8 session with and without the fields, an alias
  query, a parameter description, and a pre-1.8 session that omits them. The
  mock provider holds the descriptions and one alias and strips them before 1.8.
- `aliases` is a fact about the entry, so an entry carries no per-query match
  field and the Wright decision that entries carry no per-entry `match` stays
  intact. A client compares its query with `spelling` and `aliases`.
- Providers choose what to describe. A client renders what is present.

## Alternatives considered

- **A per-query `matched` field naming the alias that matched:** rejected. It
  makes an entry depend on the query, and it conflicts with the Wright
  decision that entries carry no per-entry `match`.
- **A `lookupDescriptions` capability:** rejected. An omitted field already
  tells the client the provider did not provide it, so negotiation adds a
  round of state and no information.
- **A description with markup or a structured parameter table:** rejected. No
  client needs more than a sentence, and plain text keeps the contract
  independent of any renderer.
