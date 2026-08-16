# Language Provider Protocol

The Language Provider Protocol (LPP) is a versioned protocol between tooling clients and long-running language provider processes in the WrightKit ecosystem.

LPP defines how clients (such as the Wright CLI, language services, or agent tools) communicate with a provider process that understands a specific source language (such as OPY or OSTW). The protocol keeps clients decoupled from compiler internals: it passes source text, positions, diagnostics, and source-level edits, without exposing provider ASTs, HIR, or Wright internal representations.

## Status

* Protocol version: **1.0** (specified in [`spec/lpp-v1.md`](spec/lpp-v1.md)).
* Repository state: initial published contract and conformance suite. The Wright client integration is tracked in [wrightkit/wright#142](https://github.com/wrightkit/wright/issues/142).

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

* Read [`spec/lpp-v1.md`](spec/lpp-v1.md) for the wire specification. A complete provider can be written using only the specification and the conformance fixtures.
* See [`conformance/README.md`](conformance/README.md) for details on running the fixtures, runner, and mock provider.

## Licensing and provenance

This repository is licensed under the MIT License (see [`LICENSE`](LICENSE)). The specification and conformance fixtures are free to use in any implementation. Providers do not need to use the MIT License for their own code.

The specification was written from scratch for WrightKit without copying code or text from upstream compilers or language services. Upstream projects like OverPy and OSTW serve as external compatibility references.

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
