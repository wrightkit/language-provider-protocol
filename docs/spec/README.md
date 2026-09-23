# Language Provider Protocol (LPP): Version 1

| | |
| --- | --- |
| Protocol versions | `1.0`, `1.1`, `1.2`, `1.3` |
| Status | Normative for protocol major version 1 |
| Transport | JSON-RPC 2.0 over stdio, newline-delimited framing |
| Conformance | `conformance/fixtures/v1/` + `conformance/runner` + `conformance/mock-provider` |
| Client integration | wrightkit/wright#142 (Wright client/runtime) |

> Decision rationale for material process and wire-boundary choices is recorded
> in the non-normative [LPP ADR registry](../adr/README.md). This
> specification remains the source of truth for the current wire contract.


## Specification map

This directory is the normative LPP v1 specification, split by concern for progressive disclosure.
Section numbering remains stable across files.

- [Transport, JSON-RPC, and session lifecycle](transport.md) — sections 3–5
- [Common data types](types.md) — section 6
- [Initialization and project requests](project-loading.md) — sections 7–8
- [Check, compile, and reconstruct](analysis-and-compilation.md) — sections 9–11
- [Symbols, references, rename, edit validation, and shutdown](symbols-and-edits.md) — sections 12–17
- [Errors and protocol evolution](errors-and-versioning.md) — sections 18–19
- [Conformance and appendices](conformance.md) — section 20 and appendices

Read only the sections relevant to the implementation task. The files together form one versioned protocol contract.

## 1. Introduction

The Language Provider Protocol (LPP) defines the contract between a **client**
(tooling such as a CLI, language services, or agent tooling) and a
**provider**: a long-running process that understands one or more source
languages used to author Overwatch Workshop content (OPY, OSTW, or any
third-party language).

LPP is a **process boundary**. A client communicates with a provider through
JSON-RPC 2.0 messages carrying source text, positions, ranges, diagnostics,
and source-oriented edits. The protocol MUST NOT expose provider-internal
representations (lexer/parser state, ASTs, HIR, or compiler IR) and MUST NOT
expose Wright or workshop-rs internal representations. Compile results cross
the boundary only as opaque **Workshop artifacts** (see
[Section 10](#10-lppcompile)).

LPP is intentionally language-agnostic: nothing in the wire format assumes a
particular source language, a particular implementation language for the
provider, or a particular client. The protocol is designed to be implementable
outside Rust; no Rust type or encoding appears in the wire format.

### 1.1 Goals

* Define a small, versioned protocol supporting the current ecosystem needs:
  checking, compilation to a Workshop artifact, reconstruction from a Workshop
  artifact, symbols/definition/references, semantic rename, and edit
  validation.
* Make source identity, positions/ranges, edit versions, errors, and refusals
  explicit.
* Keep every capability independently optional and negotiated at
  initialization.
* Provide machine-testable conformance fixtures so that a provider in any
  language can be verified.

### 1.2 Non-goals

* Dynamic-library/FFI plugin ABIs.
* Remote provider discovery or provider marketplaces.
* Generic compiler/type-system/AST APIs.
* Defining the semantics of any source language or of any Workshop artifact
  format (the artifact *envelope* is defined here; formats are owned by the
  ecosystem, see [Section 10.2](#102-artifact-formats)).
* Freezing a portable Workshop semantic IR. LPP does not interpret artifact
  content.

## 2. Conventions and terminology

### 2.1 Keywords

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY, and OPTIONAL in this document are to be interpreted as
described in RFC 2119.

### 2.2 Terminology

* **Client**: the process that spawns and communicates with a provider.
* **Provider**: the long-running process implementing LPP for one or more
  source languages.
* **Session**: one provider process lifetime, from spawn until exit.
* **Document**: a unit of source text identified by a URI, tagged with a
  language id and a version.
* **Document set**: a collection of documents supplied with a request. LPP is
  stateless with respect to document contents: every request carries the text
  it operates on.
* **Workshop artifact**: an opaque envelope (format id + content) produced by
  compilation and consumed by reconstruction.
* **Capability**: an independently optional protocol feature advertised by the
  provider during initialization.
* **Refusal**: a well-formed, machine-readable decline of a request that the
  provider understood. Refusals are a normal outcome, distinct from errors.

