# Releases

Language Provider Protocol releases are GitHub Releases that pin an immutable repository state for the normative specification, conformance fixtures, and conformance implementation.

Repository releases use semantic version tags such as `v1.1.0`. The repository/package version follows that SemVer identity. LPP wire versions remain protocol negotiation identities such as `1.0` and `1.1`; a patch repository release such as `v1.1.1` does not define a new wire version.

Release Please maintains the release PR, repository version, changelog, tag, and GitHub Release. The release PR must pass the repository's normal Rust and LPP conformance CI before it is merged and therefore before its tag and GitHub Release are published.

Releases do not publish the conformance workspace packages to crates.io. Consumers may build the conformance runner from the tagged source when needed.
