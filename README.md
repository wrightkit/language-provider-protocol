# Language Provider Protocol

The Language Provider Protocol (LPP) is a versioned protocol between tooling
clients and long-running language-provider processes in the WrightKit
ecosystem.

A **provider** is a protocol role, not the product identity of a language
repository. An independently usable language implementation such as `opy-rs`
or `del-rs` may expose an LPP provider process so Wright, editors, agents, or
other tooling clients can consume diagnostics, semantic queries, and validated
source edits without sharing compiler internals. The same implementation may
also expose its own Rust library and standalone CLI and must not require Wright
for standalone use.

LPP therefore decouples tooling integration from implementation ownership:

```text
standalone language implementation
    ├─ library / CLI
    └─ optional LPP provider process
              ↓
         tooling clients
      Wright / agents / editors
```

LPP passes source text, positions, diagnostics, semantic-query results, and
source-level edits without exposing provider ASTs/HIR or Wright internal
representations. It does not prescribe how a language implementation organizes
its frontend, compiler, reconstruction, or Workshop integration internally.

## Status

- Protocol versions: **1.0** and additive **1.1** (specified in [`spec/lpp-v1.md`](spec/lpp-v1.md)).
- Repository state: initial published contract and conformance suite.
- Wright is a client/consumer of the protocol; LPP is not a dependency from the
  language implementation back into Wright tooling internals.
- Repository releases use SemVer GitHub tags and releases; see [`RELEASE.md`](RELEASE.md).

## Repository layout

```text
AGENTS.md                    Repository ownership, routing, and validation rules
LICENSE                      MIT License
RELEASE.md                   Release identity and versioning contract
spec/lpp-v1.md               Normative LPP v1 specification
conformance/README.md        Provider conformance workflow
conformance/fixtures/v1/     Versioned JSON-RPC message fixtures
conformance/mock-provider/   Reference mock provider for x-demo-lang
conformance/runner/          Conformance runner for provider binaries
```

## Specification and conformance

- Read [`spec/lpp-v1.md`](spec/lpp-v1.md) for the wire specification. A provider
  can be implemented using only the specification and conformance fixtures.
- See [`conformance/README.md`](conformance/README.md) for the fixture runner and
  mock-provider workflow.

Conformance proves that a process speaks LPP correctly. It does **not** prove
that the underlying language implementation is semantically complete or that
Wright supports every capability of that implementation.

## Validation

```text
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
./target/release/lpp-conformance-runner --validate-only --fixtures conformance/fixtures/v1
./target/release/lpp-conformance-runner --provider ./target/release/lpp-mock-provider --fixtures conformance/fixtures/v1
```

CI runs the Rust checks and the full conformance run on every push and pull
request.

## Licensing and provenance

This repository is licensed under the MIT License. The specification and
conformance fixtures are free to use in any implementation. Implementations
that expose an LPP provider do not need to use the MIT License for their own
code.

The specification was written from scratch for WrightKit without copying code
or text from upstream compilers or language services. Upstream projects such as
OverPy and OSTW remain external compatibility references for their respective
implementations, not for the LPP protocol itself.
