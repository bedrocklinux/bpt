bpt(1)

# NAME

bpt - Bedrock Package Tool

# SYNOPSIS

*bpt* [_common-options_] <command> [_arguments_]

# DESCRIPTION

*bpt* is the Bedrock Linux package manager.

It can:

- install, remove, upgrade, downgrade, and apply package selections
- inspect installed and repository package metadata
- synchronize repository indexes and fetch repository packages
- build binary packages from build definitions
- sign and verify package-related files

Most commands operate against a target root selected with *-R* or *--root-dir*.
If no root is specified, */* is used.

# COMMON OPTIONS

*-h*, *--help*
	Display help information.

*-v*, *--version*
	Display version information.

*-y*, *--yes*
	Assume yes as the answer to all prompts and run non-interactively.

*-D*, *--dry-run*
	Show steps that would be taken without taking them.

*-N*, *--netutil-stderr*
	Print network utility stderr.

*-V*, *--skip-verify*
	Skip verifying signatures.

*-S*, *--skip-sign*
	Skip signing results.

*-P*, *--priv-key*=_PATH_
	Use the specified minisign private key.

*--priv-key-passphrase-file*=_PATH_
	Read the minisign private key passphrase from a file.

*-O*, *--out-dir*=_PATH_
	Write fetched or built output files to _PATH_.

*-R*, *--root-dir*=_PATH_
	Operate on the filesystem tree rooted at _PATH_.

# COMMANDS

## install

*bpt* install [_options_] <pkg>...

Install packages.

Each package argument may be:

- a repository package identifier
- a path to a *.bpt* binary package
- a path to a *.bbuild* build definition
- an HTTP or HTTPS URL to a *.bpt* binary package
- an HTTP or HTTPS URL to a *.bbuild* build definition

Options:

*-r*, *--reinstall*
	Reinstall already installed package(s).

## remove

*bpt* remove [_options_] <partid>...

Remove packages selected from the world file.

Options:

*-p*, *--purge*
	Also remove modified configuration files.

*-f*, *--forget*
	Forget package metadata without removing files from disk.

## upgrade

*bpt* upgrade [<pkg>...]

Upgrade installed packages.

With no package arguments, upgrade all installed packages that have an explicit installed version.
With arguments, each argument may be a repository package identifier, a package path, or a package URL.

## downgrade

*bpt* downgrade <pkg>...

Downgrade installed packages.

Each argument may be a repository package identifier, a package path, or a package URL.
Repository downgrades require an explicit target version.

## apply

*bpt* apply

Reconcile the current installed package set against the current */etc/bpt/world* file for the selected root.
This is useful after manual world-file edits or after recovering from partial operations.

## check

*bpt* check [_options_] [<partid>...]

Check installed package integrity.

With no package arguments, check all installed packages.

Options:

*-s*, *--strict*
	Treat backup file content differences as errors.

*-i*, *--ignore-backup*
	Ignore backup file content differences.

## info

*bpt* info [_options_] <pkg>...

Describe packages.

Arguments may be installed package identifiers, repository package identifiers, package paths, or package URLs.

Options:

*-i*, *--installed*
	Search installed packages.

*-r*, *--repository*
	Search repository packages.

## files

*bpt* files [_options_] <pkg>...

List files provided by packages.

Arguments may be installed package identifiers, repository binary package identifiers, package paths, or package URLs to binary packages.

Options:

*-i*, *--installed*
	Search installed packages.

*-r*, *--repository*
	Search repository packages.

## search

*bpt* search [_options_] <regex>

Search package names or descriptions.

Options:

*-n*, *--name*
	Search package names.

*-d*, *--description*
	Search package descriptions.

*-i*, *--installed*
	Search installed packages.

*-r*, *--repository*
	Search repository packages.

## list

*bpt* list [_options_]

List packages.

Options:

*-i*, *--installed*
	List installed packages.

*-r*, *--repository*
	List repository packages.

*-x*, *--explicit*
	List explicitly installed packages.

*-d*, *--dependency*
	List packages installed as dependencies.

## provides

*bpt* provides [_options_] <regex>

List packages that provide files matching a regular expression.

Options:

*-i*, *--installed*
	Search installed packages.

*-r*, *--repository*
	Search repository packages.

## sync

*bpt* sync [<index>...]

Synchronize repository package and file indexes.

With no arguments, synchronize all configured indexes. Arguments may be local *.pkgidx* and *.fileidx* paths or HTTP/HTTPS URLs.

Options:

*-f*, *--force*
	Refresh indexes even if they were checked recently.

## fetch

*bpt* fetch <partid>...

Fetch repository packages into the output directory.

## clean

*bpt* clean [_options_]

Remove cached package and source files.

With no options, both caches are cleaned.

Options:

*-p*, *--packages*
	Remove cached packages.

*-s*, *--source*
	Remove cached source files.

## build

*bpt* build [_options_] <bbuild>...

Build binary packages from build definitions.

Each argument may be a repository build-definition identifier, a local *.bbuild* path, or an HTTP/HTTPS URL to a *.bbuild* file.

Options:

*-a*, *--arch*=_ARCH_
	Target architecture.

## make-repo

*bpt* make-repo

Generate local repository artifacts, including *.bpt*, *.pkgidx*, and *.fileidx* files.

## verify

*bpt* verify <path>...

Verify signatures on files.

## sign

*bpt* sign <path>...

Sign files, replacing any existing bpt signature block.

Options:

*-n*, *--needed*
	Only sign files which do not currently pass *bpt verify*.

# FILES

*/etc/bpt/bpt.conf*
	General bpt configuration.

*/etc/bpt/make.conf*
	Build configuration.

*/etc/bpt/make.common*
	Build helper shell functions.

*/etc/bpt/repos/*
	Repository configuration files.

*/etc/bpt/world*
	List of explicitly desired packages.

*/var/lib/bpt/pkgidx/*
	Cached repository package indexes for the selected root.

*/var/lib/bpt/fileidx/*
	Cached repository file indexes for the selected root.

*/var/lib/bpt/instpkg/*
	Installed package metadata for the selected root.

# SHELL INTEGRATION

The project ships shell completion files for Bash, Zsh, and Fish.

Installed package and world-file aware completions honor *-R* and *--root-dir* when those are present on the command line.

# SEE ALSO

*bpt.conf*(5)
