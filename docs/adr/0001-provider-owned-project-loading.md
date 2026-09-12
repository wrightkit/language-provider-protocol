# ADR 0001: Provider-Owned Project Loading from an Entry Target

- Status: Accepted
- Decision date: 2026-09-02 (UTC)
- Backfilled: 2026-09-12 (UTC)
- Scope: LPP 1.1 `lpp/check` and `lpp/compile` requests

## Context

LPP 1.0 requires a client to provide every document needed by a request. That
keeps document-supplied requests stateless, but it makes the client responsible
for discovering a source-language project before it can call the provider.

The OPY integration exposed the ownership problem. `opy-rs` owns `#!mainFile`,
`#!include`, preprocessing, and the source closure rules, while Wright only
knows which path the user selected. Having Wright recursively discover those
files would duplicate OPY project-loading semantics in a client and would make
the two implementations drift.

The need and ownership boundary were recorded in [Issue #16](https://github.com/wrightkit/language-provider-protocol/issues/16)
and exercised by [wrightkit/wright#243](https://github.com/wrightkit/wright/issues/243)
and [wrightkit/opy-rs#170](https://github.com/wrightkit/opy-rs/issues/170).

## Decision

LPP 1.1 gives the provider ownership of filesystem-backed project discovery
from a client-selected entry target.

The client sends a `ProjectEntry` containing an absolute `file` URI, the
advertised language id, and a non-negative snapshot version. For `lpp/check`
and `lpp/compile`, the request contains either `documents` or `entry`, never
both. Entry requests require protocol version `1.1` and the negotiated
`projectLoading` capability. An optional `projectRoot` remains legal and
informational; it does not override provider project discovery.

The provider determines the effective project root and source closure according
to the source language's rules, loads the required files, and performs the
operation over that closure. Results use stable canonical source URIs and echo
the entry version. An invalid entry produces `invalidEntry`; an unreadable
entry or required source file produces `projectLoadFailed`; neither condition
may produce a partial successful result.

The normative contract and conformance evidence are defined by
[specification sections 6.10, 7.3, 7.4, 8.1, 10, and 19](../../spec/lpp-v1.md)
and [fixtures 36–40](../../conformance/fixtures/v1/).

## Consequences

- Wright can pass the selected entry without reproducing language-specific
  project discovery.
- A provider can apply its own project-root and source-closure rules while the
  protocol remains language-neutral.
- Providers need filesystem access for entry requests and must report loading
  failures as structured protocol errors.
- LPP 1.0 document-supplied workflows remain available, and providers without
  `projectLoading` can continue to support those workflows.
- Entry requests describe a filesystem project boundary; they do not define
  workspace synchronization, unsaved editor overlays, or package dependencies.

## Alternatives considered

### Client-provided complete closure

Rejected because it makes the client duplicate source-language project loading
and cannot preserve a single owner for OPY discovery rules.

### General workspace synchronization

Rejected as broader than the demonstrated need. The entry contract solves
provider-owned filesystem discovery without adding editor-state or package
model semantics to LPP.

### Language-specific discovery rules in LPP

Rejected because source-language repositories own syntax and project semantics.
LPP carries the entry envelope and error/result contract, not OPY or OSTW rules.

## Historical evidence

- [Issue #16](https://github.com/wrightkit/language-provider-protocol/issues/16)
  defines the goal, non-goals, ownership, and acceptance criteria.
- [PR #17](https://github.com/wrightkit/language-provider-protocol/pull/17)
  merged the implementation as commit
  [`cac74c1`](https://github.com/wrightkit/language-provider-protocol/commit/cac74c1b00596c75869c69fadab576a1d4897942),
  including the provider, runner, and fixtures.
- The final review correction in commit
  [`bafab7c`](https://github.com/wrightkit/language-provider-protocol/commit/bafab7cd13bf2c327a9a1765ee15214516e99c57)
  preserved legal `projectRoot` usage for entry requests and clarified that
  `check` or `compile` remains required alongside `projectLoading`.
