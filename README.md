# bpt

`bpt` is the Bedrock Package Tool, the package manager for Bedrock Linux.

For command-line usage, see [doc/man/bpt.1.scd](./doc/man/bpt.1.scd).

For underlying concepts, see [doc/concepts.md](./doc/concepts.md).

## Build

```sh
cargo build --release
```

See ./target/release/bpt

## Test

```sh
cargo test
```

## FAQ

### Why does Bedrock Linux need a package manager?

Originally Bedrock Linux was not intended to have its own package manager based
on the expectation it could instead leverage other distros for any package management needs.  In
practice, this was found to be inadequate:

- Less-important Bedrock components such as `brl fetch` back-ends update more
  frequently than the core Bedrock functionality, which makes it difficult to
  follow Bedrock update versions.  Separating Bedrock components out into
  independently versioned packages resolves this, but then requires a package
  manager.
- Different users need different Bedrock components; after running `brl
  tutorial`, for example, it has little remaining need on-disk.  There is thus
  value in it being independently installable and removable, which in turn requires
  a package manager.
- There is a strong demand for third-party Bedrock-specific packages.
  Packaging these for other distros results in coordination issues; some
  Bedrock Linux users have some package managers, others have others; there was
  no obvious Schelling point to target.  Officially blessing a package manager
  resolves this.

### What makes this different from other major Linux package managers?

- Bedrock-awareness in package build infrastructure, such as changing
  build-time vs run-time linking paths.
- First-class support for build time dependencies that are binaries rather than
  packages, with the intent of getting the binaries from other Bedrock strata.
  This lessens the maintenance burden; the Bedrock team does not need to package
  things such as compilers.
