# Language Provider Protocol

The Language Provider Protocol (LPP) is a versioned protocol that lets tools
such as [Wright](https://github.com/wrightkit/wright), editors, and agents talk
to language-provider processes. A provider exposes diagnostics, symbol queries,
compilation, and source edits for one language over JSON-RPC, without sharing
its internal AST.

[`opy-rs`](https://github.com/wrightkit/opy-rs) and
[`deltin-rs`](https://github.com/wrightkit/deltin-rs) are independent
compilers that can optionally run as LPP providers.

## Implementing a provider

1. Read the [specification](docs/spec/README.md). It is enough to build a
   provider.
2. Run the conformance suite against your provider:

   ```sh
   cargo build --release --bin lpp-mock-provider --bin lpp-conformance-runner
   ./target/release/lpp-conformance-runner --provider ./your-provider --fixtures conformance/fixtures/v1
   ```

See [`conformance/README.md`](conformance/README.md) for details. Passing
conformance shows the protocol is handled correctly, not that the underlying
compiler supports every language feature.

## Documentation

See [`docs/README.md`](docs/README.md) for the specification index and
architecture decisions.

## Contributing

Read [`AGENTS.md`](AGENTS.md) first.

## License

MIT. Providers built on LPP may use any license.
