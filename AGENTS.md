# AGENTS.md

This repository is part of the **WrightKit** multi-repository workspace.
Apply the workspace-level `AGENTS.md` when available, then follow this
repository's local ownership, architecture, validation, and delivery rules.

Within WrightKit, this repository owns the **Language Provider Protocol (LPP)**:
the neutral, versioned wire contract between Wright-style tooling clients and
long-running language provider processes, plus the machine-testable conformance
suite that keeps that contract honest.

This file is a concise routing and engineering guide, not a project status page
or a duplicate of the specification.

## Repository ownership

Before implementing a task, identify the semantic and product owner:

* **This repository (`language-provider-protocol`)**: Owns the LPP
  specification (`spec/`), the conformance suite (`conformance/`: fixtures,
  runner, and the reference mock provider), and the protocol's version
  negotiation contract. All protocol-visible schema changes live here.
* **Wright (`wright`)**: Owns the client/runtime side: provider discovery,
  process lifecycle, request correlation, and consuming LPP capabilities
  (tracked in wrightkit/wright#142). Wright must not re-define protocol schema
  in its own repository.
* **Language providers (OPY, OSTW, third-party)**: Own source-language syntax,
  parsers, ASTs, symbol resolution, and compiler internals. Their internals are
  never part of the wire contract.
* **`workshop-rs`**: Owns canonical Workshop semantics and any canonical
  Workshop artifact format. LPP only defines the artifact *envelope*; artifact
  format semantics are not frozen in this repository without concrete
  evidence.

Do not move responsibilities across repositories merely to simplify an
implementation.

## Hard architecture constraints

* The wire format is JSON-RPC 2.0 over stdio with newline-delimited framing.
  It must remain implementable outside Rust: no Rust types, no Rust encoding,
  and no assumption about the provider's implementation language ever appear
  in the protocol.
* LPP is a stable **process boundary**. It must not expose provider AST/HIR or
  Wright/workshop-rs internal IR as protocol data. Compile results use the
  opaque Workshop artifact envelope only.
* The protocol is source-oriented: edits, positions, ranges, and document
  versions are expressed against source text, never against compiler-internal
  representations.
* No speculative capabilities. Capability ids, methods, and fields are added
  only when a concrete ecosystem need exists, and only through the version
  negotiation contract documented in the spec.
* Conformance fixtures are normative evidence. Any protocol-visible change
  MUST land with matching fixture updates, and every fixture MUST pass against
  the reference mock provider without protocol schema changes.
* Fixtures and conformance scenarios are versioned (`conformance/fixtures/v1/`)
  so protocol evolution is testable across versions.

## Validation contract

The canonical validation commands for this repository are:

```text
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
./target/release/lpp-conformance-runner --validate-only --fixtures conformance/fixtures/v1
./target/release/lpp-conformance-runner --provider ./target/release/lpp-mock-provider --fixtures conformance/fixtures/v1
git diff --check
```

A successful conformance run against the mock provider is required before any
protocol-visible change is committed. A single build pass is not proof of
conformance; state the evidence level and boundary.

## Routing and change paths

* **Spec or schema change**: edit `spec/lpp-v1.md` and the versioned fixtures
  together; document evolution implications in the spec's version-negotiation
  section. Do not change the wire contract without fixture coverage.
* **Fixture or runner change**: keep fixtures deterministic and self-contained;
  runner behavior must not depend on provider implementation details.
* **Mock provider change**: the mock is the reference implementation of the
  conformance suite. Its language (`x-demo-lang`) must remain deliberately
  unlike OPY and DEL so that accidental two-language assumptions in the
  protocol surface as conformance failures.
* **Client-side work** (wright#142 and later): belongs in `wrightkit/wright`,
  consuming this repository's contract, never redefining it.

## Delivery

* Use Conventional Commits (`type(scope): subject`).
* Work on independent branches; deliver through PRs; do not push to `main`.
* Review the complete diff and run `git diff --check` before committing.
* Stage only task-owned files; preserve unrelated dirt.
* Never commit credentials, private runtime data, or unreviewed third-party
  material.
* Canonical repository-facing content is written in English. Preserve protocol
  identifiers exactly as defined in the spec.
