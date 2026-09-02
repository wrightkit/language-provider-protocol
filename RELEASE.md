# Releases

Language Provider Protocol releases are GitHub Releases that pin an immutable repository state for the normative specification, conformance fixtures, and conformance implementation.

Repository releases use semantic version tags such as `v1.1.0`. `version.txt` records that repository release identity. LPP wire versions remain protocol negotiation identities such as `1.0` and `1.1`; a patch repository release such as `v1.1.1` does not define a new wire version.

The Rust workspace packages under `conformance/` are implementation tools with their own package version. Their Cargo package version is not the LPP repository release version and is not published to crates.io.

Release Please maintains the release PR, `version.txt`, changelog, tag, and GitHub Release. The release PR must pass the repository's normal Rust and LPP conformance CI before it is merged and therefore before its tag and GitHub Release are published.

Consumers may build the conformance runner from tagged source when needed.
