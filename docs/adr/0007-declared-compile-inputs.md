# ADR 0007: Provider-Declared Compile Inputs

- Status: Accepted
- Decision date: 2026-10-28 (UTC)
- Backfilled: 2026-10-28 (UTC)
- Scope: LPP 1.7 entry-based `lpp/compile` results

## Context

A client that caches a compile result, such as `wright serve`
([wrightkit/wright#1005](https://github.com/wrightkit/wright/issues/1005)),
must know when the result is stale. The provider owns source loading
([ADR 0001](0001-provider-owned-project-loading.md)), so only the provider
knows which files a compile read. Without that knowledge the client hashes the
whole project directory on every request.

The OPY provider already carries a `files` table inside its artifact. That
table is inside the opaque artifact payload (spec Section 10.2), and released
OPY providers omit compile-time `.js` inputs, so a client cannot trust it. On
2026-10-28 the owner chose a provider-neutral LPP declaration over an OPY
version check
([Issue #105](https://github.com/wrightkit/language-provider-protocol/issues/105)).

## Decision

LPP 1.7 adds an independently negotiated `compileInputs` capability. When a
provider advertises `compileInputs: true`, every entry-based compile result
MUST include `compileInputs`: a sorted, duplicate-free array of absolute
`file:` URIs. The array lists every file whose content the compile read and
every directory whose listing it read, including non-source files and paths
that were probed but missing. The provider guarantees that a change to a path
that is neither listed nor an entry of a listed directory cannot change the
result.

The field is gated like `sourceIdentity` ([ADR 0003](0003-owner-selected-source-identity.md)):
a client that needs it requests `"1.7"` and requires the capability. If the
capability is false, the field is omitted. Document-supplied compile results
and results in sessions before LPP 1.7 never include it.

The conformance runner checks this rule on every actual compile response,
independent of the fixture: it rejects the field without the negotiated
capability, on a document-supplied result, or with a malformed shape, and it
rejects an entry-based result that omits the field after the capability was
negotiated.

The normative contract is in
[specification sections 7.3, 7.4, 10.2, and 19](../spec/README.md); the
evidence is [fixtures 69–70](../../conformance/fixtures/v1/).

## Consequences

- A client can hash only the declared inputs and keep a cached result while
  none of them change. A client that sees no capability keeps whole-tree
  hashing.
- The artifact stays opaque. The declaration is a result field, not artifact
  content.
- The provider must track every read the compile makes, including directory
  listings and failed probes. A provider that cannot do this advertises the
  capability as false.
- The declaration is a set of paths, not a content hash. The client still
  hashes the listed files and listings itself.

## Alternatives considered

### Completeness flag for the artifact `files` table

A boolean result field stating that the artifact's own file table is complete.
Smallest provider change for OPY. Rejected because the client must read the
artifact payload to learn the inputs, which breaks the opaque-artifact
boundary, and because a provider without such a table gains nothing.

### Input URI list without directory listings

Simpler to produce. Rejected because a provider that discovers sources by
listing a directory changes its result when a file is added there. Without the
directory in the set, the guarantee would be false for directory targets.

### Content digests per input

The provider returns a hash per input. Rejected because the client must hash
the files itself to detect later edits; provider digests add a protocol-level
hash and ordering model with no consumer need.

### Provider version check in the client

Rejected by the owner: it is a name and version heuristic, specific to one
provider, and outside the protocol contract.

## Historical evidence

- [Issue #105](https://github.com/wrightkit/language-provider-protocol/issues/105)
  states the need and constraints.
- [wrightkit/wright#1005](https://github.com/wrightkit/wright/issues/1005)
  is the consumer that needs the declaration.
