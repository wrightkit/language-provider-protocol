# ADR 0002: Provider-Owned Directory Project Targets

- Status: Accepted
- Decision date: 2026-09-12 (UTC)
- Backfilled: 2026-09-14 (UTC)
- Scope: LPP 1.2 and later entry-based `lpp/check` and `lpp/compile` requests

## Context

ADR 0001 gave the provider ownership of project discovery when the client
selects a source-file entry. That contract still required Wright to select a
file before it could ask the provider to load a project. A user may instead
select a project directory, while the source-language implementation already
owns the rules for finding its effective entry, project root, and source
closure.

The directory-target need was accepted in [Issue #32](https://github.com/wrightkit/language-provider-protocol/issues/32)
as the protocol-side dependency for [wrightkit/wright#317](https://github.com/wrightkit/wright/issues/317).

## Decision

LPP 1.2 extends the provider-owned `ProjectEntry` envelope with an optional
`kind` field. A client that selects a directory sends `kind: "directory"` and
the directory's absolute `file` URI. The provider selects the effective entry,
project root, and source closure according to the source language's rules.

The client MUST NOT enumerate the directory, parse a project manifest, or
select an entry file to reproduce that behavior. The provider MUST return
canonical source URIs for loaded documents and MUST report a structured
`projectLoadFailed` error rather than a partial result when the directory or
language-owned default entry cannot be loaded. An omitted `kind` retains the
LPP 1.1 file-entry behavior; a directory target in an LPP 1.1 session is
rejected as `invalidEntry`.

This extends the target envelope from ADR 0001 without changing its ownership
decision: source-language repositories remain authoritative for project
discovery, and LPP carries only the selected filesystem target and the
result/error contract.

The normative contract and conformance evidence are defined by
[specification sections 6.10, 7.3, 7.4, 8.1, 8.2, and 19](../../spec/lpp-v1.md)
and [fixtures 40–42](../../conformance/fixtures/v1/).

## Consequences

- Wright can pass a selected directory without implementing source-language
  discovery rules.
- Providers can choose a language-appropriate default entry and source closure
  while preserving the same `check` and `compile` result shapes.
- Directory selection is an additive LPP 1.2 protocol revision; LPP 1.1
  file-entry behavior remains wire-compatible and has an explicit version
  gate.
- Providers need filesystem access and must define the relevant directory
  loading behavior in their source-language implementation.
- The protocol still does not define workspace synchronization, unsaved editor
  overlays, package dependencies, or language-specific discovery rules.

## Alternatives considered

### Client-enumerated directory closure

Rejected because it would move project discovery into Wright and duplicate the
source-language rules that ADR 0001 deliberately assigns to providers.

### A separate directory-loading method

Rejected because a directory is another provider-owned project target for the
existing `check` and `compile` operations, not a distinct operation with a new
result model.

### Treat every target as a file

Rejected because it would require the client to guess a default entry and would
make directory selection language-specific client policy.

## Historical evidence

- [Issue #32](https://github.com/wrightkit/language-provider-protocol/issues/32)
  defines the directory-target goal, compatibility boundary, and ownership.
- [PR #32](https://github.com/wrightkit/language-provider-protocol/pull/32)
  merged the implementation as commit
  [`80a8023`](https://github.com/wrightkit/language-provider-protocol/commit/80a80235e4f58d6265abf5c8837198a96b0d21d4),
  including the LPP 1.2 specification and directory-target conformance
  fixtures.
- This decision extends [ADR 0001](0001-provider-owned-project-loading.md),
  which remains the historical record for file-entry project loading.
