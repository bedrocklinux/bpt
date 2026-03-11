//! Constants

////////////////////
// Character sets //
////////////////////

/// Bourne shell default IFS style whitespace characters
pub const WHITESPACE_CHARS: &[char] = &[' ', '\t', '\n'];

/// Characters used in semver comparison
///
/// <https://docs.rs/semver/1.0.17/semver/enum.Op.html>
pub const SEMVER_CMP_CHARS: &[char] = &['=', '<', '>', '^', '~'];

/// Characters which need to be escaped for use with Bourne shells
pub const SHELL_SPECIAL_CHARS: &[char] = &[
    '"', '\'', '\\', '$', '`', '!', '#', '&', '|', ';', '(', ')', '{', '}', '<', '>', '*', '?',
    '[', ']', '\n', '\0',
];

/// Allowed characters in a package name.
///
/// While we may open up more later, for now we're following the conservative Debian convention:
/// https://people.debian.org/~holgerw/debian-policy-with-singlehtml/debian-policy/policy-1.html
///
/// > Package names (both source and binary, see Package) must consist only of lower case letters
/// > (a-z), digits (0-9), plus (+) and minus (-) signs, and periods (.).
///
/// If/when we do open up more allowed characters, keep the following disallowed character
/// constraints in mind:
/// - `/` to ensure we can name files with the package name/version
/// - `@` as it is the pkgver field separator in a pkgid
/// - `:` as it is the arch field separator in a pkgid
/// - whitespace characters to ensure we can include it in a whitespace-separated list
/// - semver comparison characters (`=`, `<`, `>`, `^`, `~`) to ensure we can parse a
///   dependency's version
/// - Shell special characters (`"`, `'`, `\`, `$`, `\0`) to ensure we can include it
///   in a shell string
#[inline]
pub const fn is_pkgname_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '+' | '-' | '.')
}

/// Allowed characters in a package version.
///
/// While we may open up more later, for now we're following the conservative Debian convention in
/// terms of character constraints (not format):
/// https://people.debian.org/~holgerw/debian-policy-with-singlehtml/debian-policy/policy-1.html
///
/// > The version number of a package. The format is: [epoch:]upstream_version[-debian_revision].
/// >
/// > The three components here are:
/// >
/// > epoch
/// >
/// > - This is a single (generally small) unsigned integer. [...]
/// > - [...]
/// > upstream_version
/// > - [...]
/// > - The upstream_version must contain only alphanumerics and the characters . + - ~ (full stop,
/// >   plus, hyphen, tilde) [...]
/// >
/// > debian_revision
/// >
/// > - [...] It must contain only alphanumerics and the characters + . ~ (plus, full stop, tilde)
/// >   and is compared in the same way as the upstream_version is.
/// > - [...]
///
/// Note Debian allow/expects a `:` as the epoch separator.  However, we have `:` reserved as the
/// architecture separator in `pkgname@pkgver:arch`.  We use a different convention of
/// `e<number>-` as the epoch prefix.
///
/// If/when we do open up more allowed characters, keep the following constraints in mind:
/// - `/` to ensure we can name files with the package name/version
/// - `@` as it is the pkgver field separator in a pkgid
/// - `:` as it is the arch field separator in a pkgid
/// - whitespace characters to ensure we can include it in a whitespace-separated list
/// - Shell special characters (`"`, `'`, `\`, `$`, `\0`) to ensure we can include it
///   in a shell string
/// - Cannot *start* with semver comparison characters (`=`, `<`, `>`, `^`, `~`) to ensure we can
///   parse a dependency's version.  May follow after initial semver comparison split.
#[inline]
pub const fn is_pkgver_start_char_disallowed(c: char) -> bool {
    matches!(c, '=' | '<' | '>' | '^' | '~')
}

#[inline]
pub const fn is_pkgver_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '.' | '+' | '-' | '~')
}

/// Base64 encoding/decoding character set
///
/// Padding (usually `=`) is not used to save a few bytes.
pub const BASE64_CHARSET: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD_NO_PAD;

////////////////
// File paths //
////////////////

// Paths relative to the specified <ROOT_DIR> (usually /)
/// Public key configuration directory
pub const PUB_KEY_DIR_PATH: &str = "etc/bpt/keys";
/// Repository configuration directory
pub const REPOS_DIR_PATH: &str = "etc/bpt/repos";
/// General configuration file
pub const BPT_CONF_PATH: &str = "etc/bpt/bpt.conf";
/// Build configuration files
pub const MAKE_CONF_PATH: &str = "etc/bpt/make.conf";
pub const MAKE_COMMON_PATH: &str = "etc/bpt/make.common";
/// File containing target list of explicitly installed packages
pub const WORLD_PATH: &str = "etc/bpt/world";
/// Directory containing package index files pulled from repository
pub const PKGIDX_DIR_PATH: &str = "var/lib/bpt/pkgidx";
/// Directory containing file index files pulled from repository
pub const FILEIDX_DIR_PATH: &str = "var/lib/bpt/fileidx";
/// Directory containing information about currently installed packages
pub const INSTPKG_DIR_PATH: &str = "var/lib/bpt/instpkg";
/// Cache directory if running as root (Uses XDG_CACHE_HOME otherwise)
pub const ROOT_CACHE_DIR: &str = "var/cache";
/// Directory containing cached packages relative to *_CACHE_DIR
pub const PKG_CACHE: &str = "bpt/pkgs";
/// Directory containing cached source relative to *_CACHE_DIR
pub const SRC_CACHE: &str = "bpt/src";

// File paths internal to a package/tarball
/// File path within a package's embedded tarball that contains the package's metadata.
pub const TARBALL_PKGINFO_PATH: &str = ".pkginfo";
/// File path within a package's embedded tarball that represents the root of the package
pub const TARBALL_ROOT_PATH: &str = "./";

// Miscellaneous paths
/// File name of lock files within a given directory
pub const LOCK_FILE_NAME: &str = ".lock";
/// Controlling terminal device path
pub const TTY_PATH: &str = "/dev/tty";

///////////////////
// Magic Numbers //
///////////////////

pub const BBUILD_MAGIC: &[u8] = b"#!/"; // It's just a shell script
pub const BPT_MAGIC: &[u8] = b"bpt\0";
pub const INSTPKG_MAGIC: &[u8] = b"instpkg\0";
pub const FILEIDX_MAGIC: &[u8] = b"fileidx\0";
pub const PKGIDX_MAGIC: &[u8] = b"pkgidx\0";
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
pub const ZSTD_DICT_MAGIC: [u8; 4] = [0x37, 0xa4, 0x30, 0xec];

/////////////////////
// MakeBins Groups //
/////////////////////

/// @core
///
/// Common UNIX utilities.
///
/// Don't include shell builtins.
pub const CORE_MAKEBINS: &[&str] = &[
    "awk", "basename", "cat", "chmod", "cmp", "cp", "cut", "date", "diff", "dirname", "expr",
    "find", "grep", "head", "hostname", "install", "ln", "mkdir", "mv", "paste", "readlink", "rm",
    "rmdir", "sed", "sh", "sort", "tail", "tar", "tee", "touch", "tr", "uname", "uniq", "wc",
    "xargs",
];

/// @autotools
///
/// Autotool utilities which are commonly needed together
pub const AUTOTOOLS_MAKEBINS: &[&str] = &[
    "aclocal",
    "autoconf",
    "autoheader",
    "autom4te",
    "automake",
    "autoreconf",
    "libtool",
    "libtoolize",
    "m4",
];

///////////////////
// Miscellaneous //
///////////////////

/// The compiled bpt binary's instruction set architecture; assumed to be the system's local
/// architecture.  Generated at build time by `build.rs`.
pub const HOST_ARCH: crate::metadata::Arch = include!(concat!(env!("OUT_DIR"), "/host-arch.rs"));

/// Default make.conf contents to use if config file is missing
pub const MAKE_CONF_DEFAULT_CONTENTS: &[u8] = include_bytes!("../assets/default-configs/make.conf");
/// Default make.common contents to use if config file is missing
pub const MAKE_COMMON_DEFAULT_CONTENTS: &[u8] =
    include_bytes!("../assets/default-configs/make.common");

/// Name to use for default make.conf and make.common if file is missing
pub const MAKE_CONF_FILENAME: &std::ffi::CStr = c"make.conf";
pub const MAKE_COMMON_FILENAME: &std::ffi::CStr = c"make.common";

/// Common prefix stem shared by all signature versions.
/// Used by signature discovery to find any version's signature block in a file's tail.
/// Needs to be human-readable as this is appended to *.bbuild files.
pub const SIG_PREFIX_STEM: &str = "\n# bpt-sig-";

/// v1 signature prefix.  v1 = minisign Ed25519 (SignatureBones).
pub const SIG_V1_PREFIX: &str = "\n# bpt-sig-v1:";

/// End of signature.  Needs to be human-readable as this is appended to *.bbuild files.
/// This is needed in part because UNIX expects text files like *.bbuild to end in a newline.
pub const SIG_SUFFIX: &str = "\n";

/// v1 (minisign) signature sizes
///
/// Base64 no-pad of 74 bytes is always exactly 99 characters:
/// - 74 = 24×3 + 2 remaining bytes
/// - 24 full groups → 96 chars, + 2 remaining bytes → 3 chars = 99 chars
/// - No padding variation — the input length is fixed, so the output length is fixed.
const SIG_V1_BASE64_LEN: u64 = 99;
/// v1 total block:
/// - \n# bpt-sig-v1: (14) + base64 (99) + \n (1) = 114 bytes
const SIG_V1_LEN: u64 = SIG_V1_PREFIX.len() as u64 + SIG_V1_BASE64_LEN + SIG_SUFFIX.len() as u64;
const _: () = assert!(SIG_V1_LEN == 114);

/// Maximum signature block size across all supported versions.
/// Used to bound how much of the file tail we read during signature discovery.
/// Update this when adding new signature versions.
pub const SIG_LEN_MAX: u64 = SIG_V1_LEN;

/// Typically a package index indicates package locations relative to the pkgidx file itself,
/// unless this xattr is set, in which case its value is used.
pub const LOCATION_OVERRIDE_XATTR: &str = "user.bpt.location";

/// Maximum size for files expected to be small (e.g. package metadata, shell script output).
///
/// Used as a cap on `read_small_file` and similar unbounded reads to guard against OOM from
/// malformed or malicious input.  1 MiB is far more than any legitimate usage requires for
/// these files.
pub const SMALL_FILE_MAX_SIZE: usize = 1024 * 1024;

/// Maximum number of files expected in a package.
pub const PACKAGE_FILE_COUNT: usize = 1024 * 1024;
