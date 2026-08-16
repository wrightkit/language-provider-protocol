# LPP v1 Conformance Suite

This directory contains the test suite and fixtures for the Language Provider Protocol v1 wire contract (see [`../spec/lpp-v1.md`](../spec/lpp-v1.md)).

## Layout

```text
fixtures/v1/          Versioned JSON-RPC message fixtures (normative test cases)
mock-provider/        Reference provider for the "x-demo-lang" equation DSL (Rust, stdio binary)
runner/               Conformance runner that replays fixtures against any provider binary
```

* **Fixtures** (`fixtures/v1/`): one JSON file per scenario. Each scenario defines a session with request/response steps, optional CLI flags, and the expected exit code. Responses are compared after JSON parsing so key order does not matter.
* **Mock provider** (`mock-provider/`): a small Rust binary implementing the full LPP v1 surface for a demonstration language distinct from OPY and OSTW. It runs over stdio so clients (like the Wright LPP client in wrightkit/wright#142) can test against it directly.
* **Runner** (`runner/`): spawns a fresh provider process per scenario, feeds requests over stdin, validates stdout responses against expectations, and checks the process exit code.

## Running the suite

```text
cargo build --release --bin lpp-mock-provider --bin lpp-conformance-runner
./target/release/lpp-conformance-runner --validate-only --fixtures conformance/fixtures/v1
./target/release/lpp-conformance-runner --provider ./target/release/lpp-mock-provider --fixtures conformance/fixtures/v1
```

`--validate-only` checks that fixtures parse and follow the scenario schema without spawning a provider. The full run prints one line per scenario and exits with status 1 if any scenario fails.

Runner options:

| Option | Meaning |
| --- | --- |
| `--fixtures <dir>` | Fixture directory (default `conformance/fixtures/v1`). |
| `--provider <path>` | Provider binary to test. Required unless `--validate-only`. |
| `--validate-only` | Validate fixture structure only. |
| `--scope <all\|protocol\|semantics>` | Run only scenarios with the given scope. |

## Verifying a provider written in any language

LPP has no wire dependency on Rust. Any provider that reads newline-delimited JSON from standard input and writes JSON-RPC 2.0 responses to standard output can run this suite.

1. Build your provider as a stdio binary.
2. Run the protocol-scope scenarios:
   `lpp-conformance-runner --provider <your-provider> --scope protocol`
3. The semantic scenarios use `x-demo-lang` (an equation-puzzle DSL). To test a provider for another language, substitute your own language source texts and artifact payloads while keeping the protocol envelope and message sequence.

Passing this suite verifies wire protocol conformance. It does not check Workshop engine correctness or runtime performance.

## Scenario file format

```json
{
  "name": "scenario-name",
  "description": "What this scenario exercises",
  "scope": "protocol | semantics",
  "providerArgs": ["--without", "reconstruct"],
  "steps": [
    {
      "request": { "jsonrpc": "2.0", "id": 1, "method": "lpp/check", "params": { } },
      "expectResponse": { "jsonrpc": "2.0", "id": 1, "result": { } }
    }
  ],
  "expectExitCode": 0
}
```

* `scope`: `protocol` scenarios exercise transport/session/negotiation
  behavior; `semantics` scenarios exercise x-demo-lang-specific behavior
  (diagnostics content, artifact content, symbol structure).
* `providerArgs`: optional extra command-line arguments for the provider
  binary (used to exercise capability negotiation).
* Each step has exactly one of `request` (a JSON-RPC message, serialized as a
  single line) or `rawLine` (a verbatim line, used for malformed-message
  scenarios).
* `expectExitCode`: the provider's exit status after stdin is closed (default
  0).

## The reference language: x-demo-lang

`x-demo-lang` (file extension `xdl`) is deliberately shaped unlike OPY and
DEL. A document declares one equation puzzle: a start value, a target value,
named arithmetic ops, and a solution that applies ops in sequence.

```text
puzzle clean {
  target = 40
  start = 10
  ops {
    double: x => x * 2
    plus1: x => x + 1
  }
  solution = [ double, double ]
}
```

Semantics exercised by the fixtures:

* **check**: syntax errors; duplicate op names (`x-demo/duplicate-op`);
  unresolved solution references (`x-demo/unresolved-op`); missing sections
  (`x-demo/missing-section`); warnings when the solution does not reach the
  target (`x-demo/target-not-reached`) or is empty (`x-demo/empty-solution`).
* **compile**: simulates the solution and emits a puzzle evaluation sheet in
  the provider's own artifact format `x-demo/puzzle-eval-v1`. The artifact is
  an opaque envelope as far as LPP is concerned; nothing in the protocol
  interprets its content.
* **reconstruct**: canonical source text regenerated from a puzzle evaluation
  sheet.
* **symbols/definition/references/rename**: the puzzle name is a `puzzle`
  symbol; each op is an `op` symbol; solution entries are references to ops.
  Renames return source edits covering declarations and all references in the
  received document set.
* **validateEdits**: normative edit application rules from spec section 16.3:
  bounds checks, overlap detection, application, and re-parsing.

The mock provider binary accepts `--without <capability>,...` to disable
capabilities at runtime, which the capability-negotiation fixture uses.
