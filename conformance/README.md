# LPP v1 Conformance Suite

This directory contains the test suite and fixtures for the Language Provider Protocol v1 wire contract, including the LPP 1.1 file-entry, LPP 1.2 directory-target project-loading, LPP 1.3 source-identity, LPP 1.4 artifact-format negotiation, and LPP 1.5 name-lookup revisions (see [`../docs/spec/README.md`](../docs/spec/README.md)).

## Layout

```text
fixtures/v1/           Versioned JSON-RPC message fixtures (normative test cases)
fixtures/v1/sessions/  Shared initialize/shutdown handshakes spliced by `"session"`
common/                File-URI codec shared by the mock provider and runner
mock-provider/         Reference provider for the "x-demo-lang" equation DSL (Rust, stdio binary)
runner/                Conformance runner that replays fixtures against any provider binary
```

* **Fixtures** (`fixtures/v1/`): one JSON file per scenario. Each scenario defines a session with request/response steps, optional CLI flags, and the expected exit code. Responses are compared after JSON parsing so key order does not matter. The directory contains LPP 1.0, LPP 1.1, LPP 1.2, LPP 1.3, LPP 1.4, and LPP 1.5 scenarios. `fixtures/v1/sessions/` holds the standard initialize/shutdown handshake once per protocol version; a scenario whose steps don't themselves exercise the handshake sets `"session": "<version>"` and the runner splices those steps around its own.
* **Mock provider** (`mock-provider/`): a small Rust binary implementing the full LPP v1 surface for a demonstration language distinct from OPY and OSTW. It runs over stdio so clients (like the Wright LPP client in wrightkit/wright#142) can test against it directly.
* **Runner** (`runner/`): spawns a fresh provider process per scenario, feeds requests over stdin, validates stdout responses against expectations, and checks the process exit code. Expected responses compare verbatim except at leaves a fixture marks provider-supplied (see the scenario format below).

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
   Scenarios that do not exercise `x-demo-lang` semantics pass out of the box: provider identity (`serverInfo`, `languages`), negotiated capability values, `supportedProtocolVersions`, and provider prose are asserted by contract shape, not by reference-adapter values.
3. The remaining scenarios are adapter-specific by design: `semantics` scenarios exercise `x-demo-lang` content, and `providerArgs` scenarios configure the reference mock's CLI. To cover those, substitute your own language source texts, artifact payloads, and adapter configuration while keeping the protocol envelope and message sequence.

Passing this suite verifies wire protocol conformance. It does not check Workshop engine correctness or runtime performance.

## Scenario file format

```json
{
  "name": "scenario-name",
  "description": "What this scenario exercises",
  "scope": "protocol | semantics",
  "session": "1.4",
  "providerArgs": ["--without", "reconstruct"],
  "projectFiles": { "entry.xdl": "...", "support.xdl": "..." },
  "steps": [
    {
      "request": { "id": 1, "method": "lpp/check", "params": { } },
      "expectResponse": { "result": { } }
    }
  ]
}
```

* `scope`: `protocol` scenarios exercise transport/session/negotiation
  behavior; `semantics` scenarios exercise x-demo-lang-specific behavior
  (diagnostics content, artifact content, symbol structure).
* `session`: optional protocol version whose standard handshake is spliced
  around `steps` from `sessions/<version>.json` — initialize before the first
  step, shutdown after the last. A scenario must not set `session` while its
  own steps call `lpp/initialize` or `lpp/shutdown`; session-behavior tests
  (mismatch, double-init, notifications) keep the handshake explicit.
* `providerArgs`: optional extra command-line arguments for the provider
  binary (used to exercise capability negotiation). These name the reference
  mock's own CLI flags; a scenario carrying `providerArgs` is adapter-specific
  and third-party providers are not expected to run it verbatim. Its
  `expectResponse` keeps the negotiated capability map verbatim — the
  `false` leaves are the assertion the scenario exists for.
* `projectFiles`: optional relative path/content pairs that the runner writes
  to an isolated temporary project for entry-based project-loading scenarios.
  `${PROJECT_URI}` in requests and expected responses is replaced with that
  project's absolute `file:` URI.
* Each step has exactly one of `request` (a JSON-RPC message, serialized as a
  single line) or `rawLine` (a verbatim line, used for malformed-message
  scenarios).
* Envelope fields may be omitted: `jsonrpc` defaults to `"2.0"` on both
  sides, and an `expectResponse` without `id` expects the request's id (null
  for `rawLine` steps). Scenarios that exercise a non-`"2.0"` or unusual id
  keep the fields explicit.
* Inside `expectResponse`, a leaf of the form `{ "$provider": "<kind>" }`
  marks a value the spec leaves provider-supplied and asserts its contract
  shape instead of a literal:

  | Marker | Asserted shape |
  | --- | --- |
  | `nonEmptyString` | any non-empty string |
  | `boolean` | any boolean |
  | `languageList` | non-empty array of `{ "id", "extensions" }` language entries, extensions lowercase without a leading dot |
  | `protocolVersions` | non-empty array of `MAJOR.MINOR` strings |

  Marker positions are a whitelist — the provider-supplied positions named
  in [§20.1](../docs/spec/conformance.md#201-provider-supplied-leaves), and
  fixture validation enforces it, including the sibling `data.lpp.kind` and
  negotiated `protocolVersion` a position can depend on. A marked
  `capabilities` object must mark exactly the capability ids declared for
  the negotiated version: a provider may advertise any subset (absent means
  not offered), but an undeclared capability id still fails the closed-set
  check. Every other leaf keeps verbatim JSON equality; markers are not
  valid in `request`.
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
* **lookup**: the `x-demo-lang` vocabulary — section keywords, the `target`
  and `start` settings, and the arithmetic operators, which form the
  `x-demo:enum/operator` enum domain — returned as ranked entries with
  callable, enum-domain, parameter, and setting facts.

The mock provider binary accepts `--without <capability>,...` to disable
capabilities at runtime, which the capability-negotiation fixtures use. A
  project-loading scenario models the x-demo project as the `.xdl` files in the
  selected entry's directory; a real provider applies its own language rules.
