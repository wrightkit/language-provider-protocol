# LPP v1 — Transport and lifecycle

[← LPP v1 specification index](README.md)

## 3. Transport

### 3.1 Process model

The client MUST spawn the provider as a child process. The provider MUST
communicate over its standard input and standard output. The provider's
standard error is reserved for human-readable logging; it MUST NOT be used for
protocol messages.

### 3.2 Framing

Each protocol message is exactly one JSON-RPC 2.0 message serialized as a
single line of UTF-8 text terminated by LF (`0x0A`). Messages MUST NOT contain
raw newline characters; newlines inside JSON strings are escaped as `\n`.

* Writers MUST emit LF line terminators and MUST flush after each message.
* Readers MUST accept both LF and CRLF line terminators.
* Readers MUST ignore empty lines.

The client MAY terminate the provider at any time (for example by closing its
standard input or by sending a signal). When the provider's standard input
reaches end-of-file, the provider MUST exit promptly with status 0. After
sending an `lpp/shutdown` response the provider MUST exit with status 0.
Providers SHOULD also exit promptly with status 0 on SIGTERM and SIGINT.

### 3.3 Encoding and limits

All text is UTF-8. Clients and providers MUST accept messages of at least
16 MiB.

## 4. JSON-RPC conformance

Messages MUST be valid JSON-RPC 2.0 requests and responses
(https://www.jsonrpc.org/specification), with the following LPP-specific
rules:

* **Requests**: every request has an `id` (integer or string), a `method`
  string, and a `params` object. The `params` field is REQUIRED in all LPP v1
  methods; where a method takes no parameters, the client MUST send an empty
  object `{}` and the provider MUST ignore it.
* **No notifications**: LPP v1 defines no notifications. A message without an
  `id`, or with a null `id`, is a protocol violation; the provider MUST
  respond with an LPP error of kind `invalidRequest` and
  `details.reason` = `notificationNotSupported`, with `id` null in the
  response. (This is an explicit deviation from JSON-RPC notification
  semantics, chosen so that client mistakes are always observable.)
* **No batches**: clients MUST NOT send batch messages. If a provider receives
  an array message, it MUST respond with the standard JSON-RPC error
  `-32600` ("Invalid Request") and `id` null.
* **Processing order**: the provider MUST process messages in the order
  received and MUST emit responses in that same order.
* **Version field**: every request and response MUST carry `"jsonrpc": "2.0"`.

### 4.1 Standard JSON-RPC errors

The provider MUST use the standard JSON-RPC error codes, with no LPP error
data attached:

| Code | Name | When |
| --- | --- | --- |
| `-32700` | Parse error | A message line is not valid JSON. Response `id` is null. |
| `-32600` | Invalid Request | The message is not a valid JSON-RPC request (wrong `jsonrpc` value, missing `method`, non-object message, batch). |
| `-32601` | Method not found | The method name is not a known LPP v1 method. |
| `-32602` | Invalid params | The `params` value does not match the method's schema (wrong types or missing required fields). |
| `-32603` | Internal error | The provider failed internally. |

## 5. Session lifecycle

```text
client spawns provider
        |
        v
lpp/initialize  -- success --> ready
        |                       |
        | error                 | lpp/check, lpp/compile, ...
        |                       |
        v                       v
  exit (client decides)    lpp/shutdown or EOF
                                |
                                v
                        provider exits (status 0)
```

* Before a successful `lpp/initialize`, the provider MUST NOT process any
  other method; any other request MUST be answered with an LPP error of kind
  `invalidRequest` and `details.reason` = `notInitialized`.
* A second `lpp/initialize` after a successful one MUST be answered with an
  LPP error of kind `invalidRequest` and `details.reason` =
  `alreadyInitialized`.
* After the `lpp/initialize` response, the session is ready. There is no
  separate "initialized" notification in LPP v1.
* Requests arriving after `lpp/shutdown` was sent MAY be ignored by the
  provider (the client MUST NOT send any).

