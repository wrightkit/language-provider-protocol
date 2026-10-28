# Language Provider Protocol

The Language Provider Protocol (LPP) is a versioned protocol that lets tools
such as [Wright](https://github.com/wrightkit/wright), editors, and agents talk
to language-provider processes. A provider exposes diagnostics, symbol queries,
compilation, and source edits for one language over JSON-RPC, without sharing
its internal AST.

[`opy-rs`](https://github.com/wrightkit/opy-rs) ships `opy-provider`, the
LPP provider Wright uses for OverPy. The spec covers protocol versions 1.0
to 1.6.

## Implementing a provider

1. Read the [specification](docs/spec/README.md). It is enough to build a
   provider.
2. Run the protocol-scope conformance scenarios against your provider:

   ```sh
   cargo build --release --bin lpp-conformance-runner
   ./target/release/lpp-conformance-runner --provider ./your-provider --fixtures conformance/fixtures/v1 --scope protocol
   ```

   The `semantics` scenarios target the bundled mock provider.

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
