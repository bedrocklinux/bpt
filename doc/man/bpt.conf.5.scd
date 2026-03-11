bpt.conf(5)

# NAME

bpt.conf - configuration file for the Bedrock Package Tool

# DESCRIPTION

*bpt.conf* configures *bpt*(1).

The default configuration file path is */etc/bpt/bpt.conf*.

The file uses INI-like sections and *key = value* entries.

Blank lines and comments beginning with *#* are ignored.

If an option is omitted, *bpt* falls back to its built-in default.

# SECTIONS

## [general]

*default-archs*

Comma-separated preferred architectures used to resolve package identifiers
that omit *:<arch>*.

Default:
	*noarch, host*

Valid values include:

- *noarch* for architecture-independent packages
- *host* for portable packages matching the current machine architecture
- *native* to prefer building locally with machine-specific optimizations
- *bbuild* for build-definition package identifiers
- explicit architectures such as *x86_64*, *aarch64*, or *riscv64gc*

Example:
	*default-archs = noarch, native*

*pin-direct-pkgver*

Whether direct package inputs such as *./pkg.bpt* or *./pkg.bbuild* should
write a version-pinned world entry.

If *false*, direct package inputs behave like an unpinned request and can be
upgraded later. If *true*, the resulting world entry includes *@<pkgver>*.

Default:
	*false*

## [networking]

*util*

Defines a command line for fetching *http://* and *https://* URLs. The command
must contain a literal *{}* placeholder where the URL should be inserted.

Values are split on ASCII whitespace. They are not shell-parsed.

The first configured utility found in *$PATH* is used.

Default:
	*util = curl -f -L {}*
	*util = wget -O- {}*

*print-stderr*

Whether the selected network utility's stderr should be shown.

Default:
	*false*

## [build]

*unprivileged-user*

User name to switch to when *bpt* is asked to process a *.bbuild* while
running as root.

Default:
	*bpt*

*unprivileged-group*

Group name to switch to when *bpt* is asked to process a *.bbuild* while
running as root.

Default:
	*bpt*

*tmp*

Directory used for temporary build artifacts.

On Bedrock Linux this should normally be a globally visible path so tooling in
other strata can access it.

Default:
	*/tmp*

## [make-repo]

*archs*

Comma-separated architectures for which *bpt make-repo* should emit package and
index artifacts.

Default:
	*noarch, host, bbuild*

Valid values include:

- *noarch*
- *host*
- *bbuild*
- explicit architectures such as *x86_64* or *aarch64*

## [cache]

*pkg-max-days*

Maximum age in days for cached *.bpt* package files during full *bpt sync*
operations.

Use *forever* to disable age-based eviction, or *0* to evict immediately.

Default:
	*90*

*src-max-days*

Maximum age in days for cached source files during full *bpt sync* operations.

Use *forever* to disable age-based eviction, or *0* to evict immediately.

Default:
	*90*

# FILES

*/etc/bpt/bpt.conf*
	Default configuration file.

# SEE ALSO

*bpt*(1)
