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

## Bootstrapping into stratum

Acquire a temporary bootstrap binary via either:

```sh
cargo install bpt
ls -la ~/.cargo/bin/bpt
```

or

```sh
git clone https://github.com/bedrocklinux/bpt
cd bpt
cargo build --release
ls -la ./target/release/bpt
```

Once you have it available, you can have it bootstrap itself into a Bedrock Linux 0.7 "Poki" stratum via (as root):

```sh
# install into bootstrap environment
mkdir -p /bedrock/strata/bpt
bpt -yVR /bedrock/strata/bpt sync https://bedrocklinux.org/repo/0.8/main/x86_64.pkgidx https://bedrocklinux.org/repo/0.8/main/noarch.pkgidx
bpt -yVR /bedrock/strata/bpt install bedrocklinux-keys bedrocklinux-repo bpt
# enable and show stratum
brl enable bpt && brl show bpt
# delete bootstrap bpt with either:
# rm ~/.cargo/bin/bpt
# rm ./target/release/bpt
# sync self-hosting bpt's repositories
bpt sync
```

## Unprivileged Build User

If `bpt` is run as root and asked to process a `*.bbuild`, it drops to the
`[build]/unprivileged-user` and `[build]/unprivileged-group` configured in
`/etc/bpt/bpt.conf`. The shipped default config expects both to be `bpt`.

`bpt` does not currently support install hooks, so installing the package does
not automatically create that account. If you plan to build packages as root,
create it explicitly:

```sh
groupadd --system bpt
useradd --system --gid bpt --home-dir /var/lib/bpt/build --shell /usr/sbin/nologin bpt
install -d -o bpt -g bpt -m 0700 /var/lib/bpt/build
```

The `/var/lib/bpt/build` home directory keeps build-user state such as
`.cargo`, `.rustup`, and `.cache` under the existing `bpt` data directory.

If your system uses a different `nologin` path, adjust the `useradd` command
accordingly. If you do not build packages as root, this setup is not required.

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
