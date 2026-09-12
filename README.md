# Language Provider Protocol

The Language Provider Protocol (LPP) is a versioned protocol between tooling
clients and long-running language-provider processes in the WrightKit
ecosystem.

The protocol allows independent compilers (such as `opy-rs` and `del-rs`) to
expose diagnostics, symbol queries, and refactoring operations to tooling clients
(such as Wright or IDE plugins) over a clean, versioned RPC interface without
coupling their internal ASTs. Compilers remain independently usable via their own
libraries and CLIs without depending on Wright.

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

- Protocol versions: **1.0**, additive **1.1** file-entry loading, additive **1.2** directory targets, and additive **1.3** source identity for entry-based compilation (specified in [`spec/lpp-v1.md`](spec/lpp-v1.md)).
- Repository state: initial published contract and conformance suite.
- Wright is a client/consumer of the protocol; LPP is not a dependency from the
  language implementation back into Wright tooling internals.

## Repository layout

```text
AGENTS.md                    Repository ownership, routing, and validation rules
LICENSE                      MIT License
spec/lpp-v1.md               Normative LPP v1 specification
docs/adr/                    Architecture Decision Records for material LPP choices
conformance/README.md        Provider conformance workflow
conformance/fixtures/v1/     Versioned JSON-RPC message fixtures
conformance/mock-provider/   Reference mock provider for x-demo-lang
conformance/runner/          Conformance runner for provider binaries
```

## Specification and conformance

- Read [`spec/lpp-v1.md`](spec/lpp-v1.md) for the wire specification. A provider
  can be implemented using only the specification and conformance fixtures.
- Read [`docs/adr/README.md`](docs/adr/README.md) for the registry and rationale
  behind material process and wire-boundary decisions.
- See [`conformance/README.md`](conformance/README.md) for the fixture runner and
  mock-provider workflow.

Passing conformance confirms that an implementation handles protocol messages
correctly. It does not certify that the underlying compiler supports every
syntax feature of the language.

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
