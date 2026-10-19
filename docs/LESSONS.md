# Lessons

Short review queue of reusable failure classes from merged LPP work. Each entry names a prevention action and the evidence behind it. Promote an entry into a regression test, an owner contract, or an existing engineering standard, then remove it here. The procedure is the `merged-pr-retro` skill in `wrightkit/.agents`.

### A version-gated feature must define every negotiated version state

- **Rule:** Adding a version-gated field or method requires enumerating what each negotiated protocol version exposes, and the gating must agree with the unknown-field and negotiation rules already in the spec.
- **Why:** Two protocol PRs merged with the same inconsistency shape — version-gated fields unreconciled with the unknown-field rule, and `acceptedArtifactFormats` left undefined for 1.4-or-later sessions. Both were caught only in review.
- **Prevention home:** conformance fixtures exercising each gated surface at each negotiated version; the spec's version-negotiation section.
- **Incidents:** [#40](https://github.com/wrightkit/language-provider-protocol/pull/40), [#46](https://github.com/wrightkit/language-provider-protocol/pull/46) — observed 2026-09-24, 2026-10-05

### A parity refactor must pin the baseline behavior it preserves

- **Rule:** Rewriting a parser, recovery, or matching path requires pinning the baseline's observable edge behavior in tests first; a refactor that changes observable recovery silently drops normative behavior.
- **Why:** Independent review found the parse rewrite dropped the baseline's second `recover_line()` — the reference oracle's normative skip after a malformed op-parameter ident.
- **Prevention home:** baseline-pinning unit tests for rewritten paths (`malformed_op_declaration_skips_following_line`, #43).
- **Incidents:** [#43](https://github.com/wrightkit/language-provider-protocol/pull/43) — observed 2026-10-01

### The reference implementation needs adversarial-input coverage, not only happy-path fixtures

- **Rule:** The mock provider and runner that real providers are measured against must reject malformed input, return spec error codes, and stay deterministic under truncation, non-UTF-8 bytes, and out-of-range values.
- **Why:** One adversarial audit found ~20 defects the conformance suite never exercised — a panic on non-UTF-8 input, an inverted phantom-line range, unsorted diagnostics and text edits, silent EOF truncation.
- **Prevention home:** `wrightkit/.github` `docs/robustness-testing.md`; the audited cases now covered in the suite (#86).
- **Incidents:** [#86](https://github.com/wrightkit/language-provider-protocol/pull/86) fixing audit issues #62–#83 — observed 2026-10-15
