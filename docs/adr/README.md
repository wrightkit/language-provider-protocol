# LPP Architecture Decision Records

This directory records material decisions about the LPP process and wire
boundary. The normative contract remains in [`spec/lpp-v1.md`](../spec/README.md);
ADRs preserve the rationale, alternatives, and consequences that should not be
inferred from the wire schema alone.

ADR files use a zero-padded sequence and a short decision slug. Each record
states its status, the historical decision date, and the date on which it was
backfilled. An ADR is not required for individual fields, fixtures, release
metadata, implementation details, or other changes that do not alter a
material process or wire-boundary decision.

## Registry

| ADR | Decision | Status | Decision date (UTC) | Backfilled (UTC) | Evidence |
| --- | --- | --- | --- | --- | --- |
| [0001](0001-provider-owned-project-loading.md) | Provider-owned project loading from a client-selected entry target | Accepted | 2026-09-02 | 2026-09-12 | [Issue #16](https://github.com/wrightkit/language-provider-protocol/issues/16), [PR #17](https://github.com/wrightkit/language-provider-protocol/pull/17) |
| [0002](0002-directory-project-targets.md) | Provider-owned directory project targets | Accepted | 2026-09-12 | 2026-09-14 | [PR #32](https://github.com/wrightkit/language-provider-protocol/pull/32) |
| [0003](0003-owner-selected-source-identity.md) | Owner-selected source identity for entry compilation | Accepted | 2026-09-13 | 2026-09-14 | [PR #34](https://github.com/wrightkit/language-provider-protocol/pull/34) |
| [0004](0004-artifact-format-negotiation.md) | Client-stated artifact format negotiation for `lpp/compile` | Accepted | 2026-09-24 | 2026-09-24 | [Issue #39](https://github.com/wrightkit/language-provider-protocol/issues/39), [workshop-rs#271](https://github.com/wrightkit/workshop-rs/issues/271) |

## Post-baseline audit

The audit baseline is the initial LPP v1 contract merged by [PR #2](https://github.com/wrightkit/language-provider-protocol/pull/2)
on 2026-08-16. The audit covers repository changes through commit
[`a9e26a4`](https://github.com/wrightkit/language-provider-protocol/commit/a9e26a4168185b2fd89ea4293fbc89664e574536a)
on 2026-09-09. The classification is limited to decisions that could affect
the process or wire boundary; it does not turn every repository change into an
ADR.

| Post-baseline area | Classification | Rationale |
| --- | --- | --- |
| Provider as an integration role ([Issue #10](https://github.com/wrightkit/language-provider-protocol/issues/10), [PR #10](https://github.com/wrightkit/language-provider-protocol/pull/10)) | Already explained | Documentation clarified the provider role and its separation from language implementation ownership; it made no wire or process change. |
| Entry-based project loading ([Issue #16](https://github.com/wrightkit/language-provider-protocol/issues/16), [PR #17](https://github.com/wrightkit/language-provider-protocol/pull/17)) | Backfill-required; see [ADR 0001](0001-provider-owned-project-loading.md) | LPP 1.1 changed who owns project discovery and added a new request shape, capability, and versioned negotiation behavior. |
| Release lifecycle and version metadata ([Issue #18](https://github.com/wrightkit/language-provider-protocol/issues/18), [PR #20](https://github.com/wrightkit/language-provider-protocol/pull/20), [PR #22](https://github.com/wrightkit/language-provider-protocol/pull/22), [PR #23](https://github.com/wrightkit/language-provider-protocol/pull/23)) | Non-ADR detail | These changes govern repository releases and explicitly do not change LPP wire semantics. |
| CI, dependency, conformance-tooling, artifact-policy, and documentation maintenance ([PR #3](https://github.com/wrightkit/language-provider-protocol/pull/3), [PR #4](https://github.com/wrightkit/language-provider-protocol/pull/4), [PR #5](https://github.com/wrightkit/language-provider-protocol/pull/5), [PR #6](https://github.com/wrightkit/language-provider-protocol/pull/6), [PR #8](https://github.com/wrightkit/language-provider-protocol/pull/8), [PR #9](https://github.com/wrightkit/language-provider-protocol/pull/9), [PR #12](https://github.com/wrightkit/language-provider-protocol/pull/12), [PR #13](https://github.com/wrightkit/language-provider-protocol/pull/13), [PR #14](https://github.com/wrightkit/language-provider-protocol/pull/14), [PR #24](https://github.com/wrightkit/language-provider-protocol/pull/24), [PR #25](https://github.com/wrightkit/language-provider-protocol/pull/25), [PR #28](https://github.com/wrightkit/language-provider-protocol/pull/28), [PR #29](https://github.com/wrightkit/language-provider-protocol/pull/29)) | Non-ADR detail | No change to client/provider responsibility, session/process semantics, version negotiation, capability negotiation, or the wire schema was identified. |

Within the reviewed history through commit
[`a9e26a4`](https://github.com/wrightkit/language-provider-protocol/commit/a9e26a4168185b2fd89ea4293fbc89664e574536a),
the audit identified no additional material process or wire-boundary decision
requiring ADR backfill.

## Follow-up audit

The follow-up audit covers accepted changes after the original baseline through
commit [`83f6d37`](https://github.com/wrightkit/language-provider-protocol/commit/83f6d376652db9bce090adba3b23c2dec0d0abaf)
on 2026-09-13. It keeps the original audit scope and records only changes that
materially affect the LPP process or wire boundary.

| Post-audit area | Classification | Rationale |
| --- | --- | --- |
| Directory project targets ([PR #32](https://github.com/wrightkit/language-provider-protocol/pull/32)) | Backfill-required; resolved by [ADR 0002](0002-directory-project-targets.md) | LPP 1.2 added a directory target shape and version gate while preserving the provider-owned project discovery decision in ADR 0001. |
| Owner-selected source identity ([PR #34](https://github.com/wrightkit/language-provider-protocol/pull/34)) | Backfill-required; resolved by [ADR 0003](0003-owner-selected-source-identity.md) | LPP 1.3 added a negotiated compile-result identity so clients can preserve provider-selected source identity without changing LPP 1.1 or 1.2 responses. |
| Release metadata for LPP 1.3 ([PR #35](https://github.com/wrightkit/language-provider-protocol/pull/35)) | Non-ADR detail | The release changed repository version metadata and did not change the protocol process or wire contract. |

The follow-up audit finds no material process or wire-boundary decision through
commit `83f6d37` that remains without a discoverable ADR classification.
