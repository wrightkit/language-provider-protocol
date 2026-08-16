# LPP v1 Conformance Suite

This directory contains the machine-testable evidence for the Language
Provider Protocol v1 wire contract (see `../spec/lpp-v1.md`).

## Layout

```text
fixtures/v1/          Versioned JSON-RPC message fixtures (normative evidence)
mock-provider/        Reference provider for the "x-demo-lang" puzzle/equation language (Rust, stdio binary)
runner/               Conformance runner that replays fixtures against any provider binary
```

* **Fixtures** (`fixtures/v1/`): one JSON file per scenario. Each scenario is a
  session: a list of request/response steps, optional provider arguments, and
  the expected provider exit code. Responses are compared exactly (after JSON
  parsing, so key order does not matter).
* **Mock provider** (`mock-provider/`): a small Rust binary implementing the
  full LPP v1 surface for a deliberately non-OPY/DEL language. It is spawnable
  as a stdio process, so client-side integration (for example the Wright LPP
  client in wrightkit/wright#142) can use it end-to-end.
* **Runner** (`runner/`): spawns a fresh provider process per scenario, writes
  each request line, reads the response line, compares it to the expected
  response, closes stdin, and checks the exit code.

## Running the suite

```text
cargo build --release --bin lpp-mock-provider --bin lpp-conformance-runner
./target/release/lpp-conformance-runner --validate-only --fixtures conformance/fixtures/v1
./target/release/lpp-conformance-runner --provider ./target/release/lpp-mock-provider --fixtures conformance/fixtures/v1
```

The `--validate-only` mode checks that every fixture is structurally valid
without spawning a provider. The full run reports one line per scenario and
exits non-zero if any scenario fails.

Runner options:

| Option | Meaning |
| --- | --- |
| `--fixtures <dir>` | Fixture directory (default `conformance/fixtures/v1`). |
| `--provider <path>` | Provider binary to test. Required unless `--validate-only`. |
| `--validate-only` | Validate fixture structure only. |
| `--scope <all\|protocol\|semantics>` | Run only scenarios with the given scope. |

## Verifying a provider written in any language

LPP deliberately has no Rust dependency on the wire: any provider that reads
newline-delimited JSON from stdin and writes JSON-RPC 2.0 responses to stdout
can be verified with this suite.

1. Build your provider as a stdio binary.
2. Run the protocol-scope scenarios:
   `lpp-conformance-runner --provider <your-provider> --scope protocol`
3. The semantic scenarios encode the reference language `x-demo-lang` (an
   equation-puzzle DSL). A provider for a different language passes those by
   replacing the document texts and artifact contents with its own language's
   inputs while keeping the protocol-level expectations unchanged. The
   x-demo-lang scenarios serve as the reference for writing such fixtures.

Conformance proves wire-contract conformance only: it does not prove Workshop
semantic correctness, game runtime behavior, or performance.

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
