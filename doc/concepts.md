# bpt concepts

This document is a terse reference for the concepts that show up repeatedly in
`bpt` commands and configuration.

## Package identifiers

### `pkgid`

A `pkgid` fully identifies one package build:

`pkgname@pkgver:arch`

Example:

`htop@3.4.1:x86_64`

Its fields are:

- `pkgname`: package name
- `pkgver`: package version
- `arch`: package architecture

A `pkgid` always has all three fields.

### `partid`

A `partid` is a partial package identifier. It always has a package name, and
may also include a version and/or architecture:

- `htop`
- `htop@3.4.1`
- `htop:x86_64`
- `htop@3.4.1:x86_64`

`bpt` resolves missing fields from context:

- installed packages
- repository contents
- `/etc/bpt/bpt.conf`'s `default-archs`

In practice:

- `pkgid` names one exact package
- `partid` describes what package the user means and lets `bpt` fill in the rest

## Architectures

### Named `Arch`

Named architectures are the usual explicit architecture values such as:

- `x86_64`
- `aarch64`
- `riscv64gc`
- `noarch`

`noarch` means architecture-independent content such as scripts or data files.

### `Arch::host`

`host` means a portable binary package suitable for the current machine's
architecture.

Example on an x86_64 machine:

- `host` resolves to `x86_64`

This is still meant to be portable across machines of that architecture.

### `Arch::native`

`native` means "build from source with non-portable optimizations tuned for this
specific machine."

`native` is not a repository package format. It is a build request. A package
built as `native` is built locally from a `*.bbuild`.

### `Arch::host` vs `Arch::native`

- `host`: prefer a portable prebuilt package for this architecture
- `native`: prefer building locally for machine-specific optimization

## What `bpt install` can take

`bpt install` is the clearest example of `bpt`'s input model.

It can take:

- A file path to a prebuilt binary package:
  - `bpt install ./htop@3.4.1:x86_64.bpt`
  - `bpt` opens it and installs it
- A file path to a package build definition:
  - `bpt install ./htop@3.4.1.bbuild`
  - `bpt` opens it, builds it, then installs it
- A URL to a prebuilt binary package:
  - `bpt install https://example/repo/htop@3.4.1:x86_64.bpt`
  - `bpt` downloads it, then installs it
- A URL to a package build definition:
  - `bpt install https://example/repo/htop@3.4.1.bbuild`
  - `bpt` downloads it, builds it, then installs it
- A `partid` for a repository binary package:
  - `bpt install htop`
  - `bpt` looks it up in configured repositories, downloads it, then installs it
- A `partid` for a repository build definition:
  - if the repository only has `htop@...:bbuild`, or `native` is preferred and a
    `bbuild` is available, `bpt` looks it up, downloads it, builds it, then
    installs it

Other commands accept similar mixtures of:

- file paths
- URLs
- repository package identifiers

`bpt install` is just the most common case.

## Building native-optimized packages by default

By default, `bpt` configuration favors pre-built, portable binaries.  If you
prefer, it can be reconfigured to build packages locally optimized for your
specific hardware.  To do so:

1. Enable repository `bbuild` indexes in `/etc/bpt/repos/bedrock`

```ini
# Enable bbuild indexes
# Provides instructions on how to build packages
https://bedrocklinux.org/repo/0.8/main/bbuild.pkgidx
https://bedrocklinux.org/repo/0.8/community/bbuild.pkgidx

# Optionally, either enable or disable pkgidx indexes.
# Disabling them ensures you always get from-source.
# Enabling them provides a fall-back if you can't build a package.
https://bedrocklinux.org/repo/0.8/main/x86_64.pkgidx
https://bedrocklinux.org/repo/0.8/community/x86_64.pkgidx

# Optionally, leave fileidx indexes.
# Used for `bpt files` and `bpt provides` look-ups without needing to
# build/install a package first.
https://bedrocklinux.org/repo/0.8/main/x86_64.fileidx
https://bedrocklinux.org/repo/0.8/community/x86_64.fileidx
```

2. Configure `/etc/bpt/bpt.conf` to favor the "native" architecture

```ini
# Retain `noarch`, which can't be set to native
[general]
default-archs = noarch, native
```

## World file

The world file is:

`/etc/bpt/world`

It records the packages the user explicitly wants installed.

Each entry is a `partid`, not necessarily a fully pinned `pkgid`.

Examples:

- `htop`
- `htop:x86_64`
- `htop@3.4.1:x86_64`

This is desired state, not a list of every installed package.

Dependencies are usually not written there unless the user explicitly asked for
them.

### `bpt apply`

`bpt apply` treats the current world file as authoritative:

1. read `/etc/bpt/world`
2. resolve dependencies
3. diff that desired package set against the installed package set
4. apply the difference

Use `bpt apply` when:

- the world file was edited manually
- a previous operation only partially completed
- you want to reconcile installed state to current desired state without changing
  the world file itself

## Explicit vs dependency packages

An explicit package is one that appears in the world file.

A dependency package is installed only because something else needs it.

This distinction matters for:

- `bpt list --explicit`
- `bpt list --dependency`
- whether removing one package should also remove now-unneeded dependencies

## Repository indexes

### `pkgidx`

A package index (`*.pkgidx`) lists packages available in a repository.

It maps package identifiers to package metadata and repository locations.

It is used for tasks such as:

- `bpt install htop`
- `bpt list --repository`
- `bpt info htop`

It can point to:

- prebuilt `*.bpt` packages
- `*.bbuild` package build definitions

### `fileidx`

A file index (`*.fileidx`) maps packages to the file paths they provide.

It is used for tasks such as:

- `bpt provides`
- repository file lookup

### Repo indexes vs caches

Repository indexes are metadata about what the repository contains.

They are not packages or source code.

In practice:

- `bpt sync` refreshes repository indexes
- `bpt clean` removes cached packages and/or cached source
- `bpt clean` does not remove installed packages

## Package cache

The package cache stores downloaded package artifacts such as:

- `*.bpt`
- downloaded `*.bbuild`

It avoids downloading the same package repeatedly.

This is separate from:

- repository indexes
- installed package metadata
- the world file

## Source cache

The source cache stores downloaded upstream source material used while building
packages from `*.bbuild`.

This is also separate from:

- repository indexes
- installed packages
- the world file

## Installed state vs caches vs world

These are different layers:

- world file:
  - what the user explicitly wants
- installed state:
  - what is actually installed under the target root
- repository indexes:
  - metadata describing what repositories offer
- package cache:
  - downloaded package/build-definition artifacts
- source cache:
  - downloaded build source material

Keeping them distinct helps explain the main maintenance commands:

- `bpt apply`: reconcile installed state to the world file
- `bpt sync`: refresh repository index metadata
- `bpt clean`: remove package/source cache entries

## `.bptnew` files

Some package files are marked as backup files, typically local configuration
files.

If `bpt` needs to install such a file but an existing on-disk file should not be
overwritten, it writes the incoming version as:

`<path>.bptnew`

Example:

- package wants to install `etc/foo.conf`
- user already has a local `etc/foo.conf`
- `bpt` writes `etc/foo.conf.bptnew`

This lets the user compare or merge the new packaged version manually.

If the incoming backup file is identical to the current on-disk file, `bpt` does
not create a `.bptnew`.
