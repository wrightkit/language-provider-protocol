# ADR 0003: Owner-Selected Source Identity for Entry Compilation

- Status: Accepted
- Decision date: 2026-09-13 (UTC)
- Backfilled: 2026-09-14 (UTC)
- Scope: LPP 1.3 entry-based `lpp/compile` results

## Context

ADR 0002 lets a client select a directory while the provider chooses the
effective source entry. A client can therefore request compilation without
knowing which source file the provider selected. The entry `version` identifies
the client's filesystem snapshot, but it is not a content hash, and the opaque
artifact does not expose source identity. Reconstructing an identity in Wright
would either require duplicating provider selection or hashing a source the
client did not select.

The source-identity need was accepted in [Issue #34](https://github.com/wrightkit/language-provider-protocol/issues/34)
as the protocol-side dependency for preserving source identity in
[wrightkit/wright#317](https://github.com/wrightkit/wright/issues/317).

## Decision

LPP 1.3 adds an independently negotiated `sourceIdentity` capability for
`lpp/compile`. When a provider advertises `sourceIdentity: true`, an
entry-based compile result MUST include `sourceIdentity`: a lower-case
SHA-256 hexadecimal digest of the provider-selected primary source text.
The provider owns effective entry selection, including for directory targets.

A client that requires this identity MUST request protocol version `1.3` and
require the capability during initialization. Providers may advertise the
capability as false and still support entry-based compilation; in that case
the field is omitted. LPP 1.1 and 1.2 compile results MUST NOT include the
field, preserving their released wire contracts. Document-supplied compile
requests may omit the field.

The identity is a deterministic digest of the selected primary source text. It
does not identify the complete source closure, attest semantic equivalence, or
replace the provider's source-loading and entry-selection responsibilities.

The normative contract and conformance evidence are defined by
[specification sections 6.10, 7.3, 7.4, 8.2, 10, and 19](../../spec/lpp-v1.md)
and [fixtures 43–44](../../conformance/fixtures/v1/).

## Consequences

- Wright can preserve the identity of a provider-selected source without
  reproducing directory discovery or reading a guessed entry file.
- Providers that can provide the digest expose it through explicit LPP 1.3
  negotiation; providers that cannot remain valid entry-compilation providers.
- The version boundary prevents an optional result field from silently changing
  released LPP 1.1 or 1.2 responses.
- The provider hashes the selected source text and must keep that identity
  associated with the compile result it returns.
- Consumers must not interpret the digest as a project-wide content hash or a
  semantic/compiler-result hash.

## Alternatives considered

### Client-computed identity

Rejected because the client does not know the provider-selected primary source
for a directory target and must not duplicate source-language discovery.

### Hash the complete source closure

Rejected because it would require a new protocol-level ordering and identity
model for language-owned project files. The demonstrated consumer need is the
identity of the provider-selected primary source.

### Reuse the entry version or artifact identity

Rejected because the entry version is client bookkeeping rather than content
identity, while artifact formats are opaque and may not have a source-derived
hash.

### Add an unnegotiated optional result field to LPP 1.1 or 1.2

Rejected because clients could not require the field reliably and providers
conforming to the released contracts would otherwise become incompatible with
new consumers.

## Historical evidence

- [Issue #34](https://github.com/wrightkit/language-provider-protocol/issues/34)
  defines the source-identity goal and its relationship to the directory-target
  workflow.
- [PR #34](https://github.com/wrightkit/language-provider-protocol/pull/34)
  merged the implementation as commit
  [`7456e62`](https://github.com/wrightkit/language-provider-protocol/commit/7456e62b98ae451655c992c3d4c99e5e53838f42),
  including the LPP 1.3 specification and conformance fixtures.
- This decision preserves the provider-ownership boundary established by
  [ADR 0001](0001-provider-owned-project-loading.md) and extended by
  [ADR 0002](0002-directory-project-targets.md).
