# Language Provider Protocol

A neutral, versioned protocol contract between tooling clients and long-running
language provider processes, owned as a standalone contract inside the
WrightKit ecosystem.

The Language Provider Protocol (LPP) defines how a client (for example the
Wright CLI, language services, or agent tooling) talks to a provider process
that understands one source language (OPY, OSTW, or any third-party language)
without the client knowing anything about that language's compiler internals.
The protocol carries source text, positions, diagnostics, and source-oriented
edits; it never exposes provider AST/HIR or Wright/workshop-rs internal IR.

## Status

* Protocol version: **1.0** (specified in [`spec/lpp-v1.md`](spec/lpp-v1.md)).
* Repository state: first published revision of the contract and conformance
  suite. The Wright client integration that consumes this contract is tracked
  in [wrightkit/wright#142](https://github.com/wrightkit/wright/issues/142).

## Repository layout

```text
AGENTS.md                    Repository ownership, routing, and validation rules
LICENSE                      MIT License
spec/lpp-v1.md               Normative LPP v1 specification (protocol version 1.0)
conformance/README.md        How the conformance suite works and how to verify a provider
conformance/fixtures/v1/     Versioned JSON-RPC message fixtures (normative evidence)
conformance/mock-provider/   Reference mock provider for "x-demo-lang" (Rust, stdio binary)
conformance/runner/          Conformance runner that replays fixtures against any provider binary
```

## Specification and conformance

* Read the specification first: [`spec/lpp-v1.md`](spec/lpp-v1.md). It is
  intended to be implementable by someone reading only the spec and the
  conformance fixtures.
* The conformance suite (fixtures + runner + mock provider) is documented in
  [`conformance/README.md`](conformance/README.md).

## Licensing and provenance

This repository is licensed under the MIT License (see [`LICENSE`](LICENSE)),
matching the permissive license of the WrightKit `workshop-rs` core. The
protocol contract, specification text, and conformance fixtures are intended
to be freely reusable by any provider implementation in any language; a
provider is not required to adopt MIT licensing for its own code.

The specification was written clean-room for WrightKit. It does not copy
implementation text from any upstream compiler or language service; upstream
projects (for example OverPy, OSTW) serve only as external compatibility
references in the wider ecosystem.

## Validation

```text
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
./target/release/lpp-conformance-runner --validate-only --fixtures conformance/fixtures/v1
./target/release/lpp-conformance-runner --provider ./target/release/lpp-mock-provider --fixtures conformance/fixtures/v1
```

CI runs the Rust checks and the full conformance run on every push and pull
request. See `.github/workflows/ci.yml`.
