//! Error handling

use crate::metadata::*;
use camino::Utf8PathBuf;

type FileType = &'static str;
// While a proper `enum Field` would a bit of typo protection, the complexity is eschewed here.
type Field = &'static str;
// Many errors relate to some file which may not have an associated file path.  The given file may
// be, for example, a remote file downloaded from a URL to an anonymous O_TMPFILE.  Thus, the
// source field of the error message may be a generic `String` rather than a path type.
type Loc = String;

/// Top-level error.  Should have all the context needed to present a good error message.
#[derive(thiserror::Error, Debug)]
pub enum Err {
    // CLI/command errors
    #[error(
        "`bbuild` is a package build definition, not a binary package architecture which can be built."
    )]
    BuildBbuild,
    #[error(
        "`bpt build` cannot output non-portable `:native` packages.  However, `bpt install` can build and install them in one go."
    )]
    BuildNative,
    #[error("Unable to build `{0}`; see build artifacts at `{1}`")]
    BuildPkg(String, Utf8PathBuf),
    #[error("Cannot order package builds due to build dependency cycle: {0}")]
    BuildDependencyCycle(String),
    #[error("Cannot build `{0}` at `--out-dir {1}`, as something already exists at `{2}`")]
    BuildOutputExists(Utf8PathBuf, Utf8PathBuf, Utf8PathBuf),
    #[error("Invalid {0} field: {1}")]
    InputFieldInvalid(Field, String),
    #[error("Invalid regular expression `{0}`: {1}")]
    InvalidRegex(String, regex_lite::Error),
    #[error("No *.bbuild files found in `{0}`")]
    MakeRepoNoBbuilds(Utf8PathBuf),
    #[error("Cannot build `{0}` for any configured [general]/default-arch value in bpt.conf.")]
    NoDefaultArchForBbuild(PkgId),
    #[error(
        "Cannot build `{0}` because required build-time binaries were not found in $PATH: {1}. Install them, potentially with another package manager or another Bedrock stratum."
    )]
    MakeBinsMissingInPath(Loc, String),
    #[error("Unable to locate package which supplies dependency `{0}` for `{1}`")]
    UnableToLocateDependency(Box<Depend>, PkgId),
    #[error("Unable to locate available package `{0}`")]
    UnableToLocateAvailablePkg(PartId),
    #[error("Unable to locate installed package `{0}`")]
    UnableToLocateInstalledPkg(PartId),
    #[error("Unable to locate repository package `{0}`")]
    UnableToLocateRepositoryPkg(PartId),
    #[error("Cannot resolve installed package dependencies due to runtime dependency cycle: {0}")]
    RuntimeDependencyCycle(String),
    #[error("Cannot remove `{0}` because it does not match any world entry")]
    RemovePkgNotExplicit(PartId),
    #[error("Cannot downgrade dependency-only package `{0}`")]
    DowngradeDependencyPkg(PartId),
    #[error("Dependency `{0}` for `{1}` lacks a populated architecture")]
    DependArchMissing(Box<Depend>, PkgId),
    #[error("File path conflict at `{0}` between `{1}` and `{2}`")]
    InstallConflict(Utf8PathBuf, Box<PkgId>, Box<PkgId>),
    #[error("Installed package integrity check failed:\n{0}")]
    CheckFailed(String),

    // Location errors
    #[error("`{0}` is not a valid filepath to a bpt")]
    InvalidBptPath(Loc),
    #[error("`{0}` is not a valid http:// or https:// URL to a bpt")]
    InvalidBptUrl(Loc),
    #[error("`{0}` is not a valid http(s) URL, filepath, or pkgid to a bpt")]
    InvalidBptPathUrlRepo(Loc),
    #[error("`{0}` is not a valid filepath to a bbuild")]
    InvalidBbuildPath(Loc),
    #[error("`{0}` is not a valid http:// or https:// URL to a bbuild")]
    InvalidBbuildUrl(Loc),
    #[error("`{0}` is not a valid http(s) URL, filepath, or pkgid to a bbuild")]
    InvalidBbuildPathUrlRepo(Loc),
    #[error("`{0}` is not a valid http(s) URL or filepath to a pkgidx or fileidx")]
    InvalidIdxPathUrl(Loc),
    #[error("`{0}` is not a valid http(s) URL to a pkgidx or fileidx")]
    InvalidIdxUrl(Loc),
    #[error("`{0}` is not a valid filepath to a pkgidx or fileidx")]
    InvalidIdxPath(Loc),
    #[error("`{0}` is not a valid filepath to a bpt or bbuild")]
    InvalidPkgPath(Loc),
    #[error("`{0}` is not a valid http(s) URL to a bpt or bbuild")]
    InvalidPkgUrl(Loc),
    #[error("`{0}` is not a valid http(s) URL, filepath, or pkgid to a bpt or bbuild")]
    InvalidPkgPathUrlRepo(Loc),
    #[error("`{0}` is not a valid http:// or https:// URL")]
    InvalidUrl(Loc),

    // I/O errors
    #[error("Unable to close pipe: {0}")]
    ClosePipe(std::io::Error),
    #[error("Unable to create anonymous (O_TMPFILE) file within `{0}`: {1}")]
    CreateAnon(Loc, std::io::Error),
    #[error("Unable to create directory `{0}`: {1}")]
    CreateDir(Loc, std::io::Error),
    #[error("Unable to create file `{0}`: {1}")]
    CreateFile(Loc, std::io::Error),
    #[error("Unable to create memfd: {0}")]
    CreateMemFd(std::io::Error),
    #[error("Unable to create pipe: {0}")]
    CreatePipe(std::io::Error),
    #[error("Unable to change ownership of `{0}`: {1}")]
    Chown(Loc, std::io::Error),
    #[error("Unable to dup file descriptor: {0}")]
    Dup(nix::errno::Errno),
    #[error("Unable to call fcntl on `{0}`: {1}")]
    Fcntl(Loc, std::io::Error),
    #[error("Unable to fork process: {0}")]
    Fork(std::io::Error),
    #[error("Unable to flush stdout: {0}")]
    FlushStdout(std::io::Error),
    #[error("Unable to read xattrs on file at {0}: {1}")]
    GetXattr(Loc, std::io::Error),
    #[error("Unable to link file descriptor to `{0}`: {1}")]
    Link(Loc, std::io::Error),
    #[error("Unable to lock file `{0}`: {1}")]
    Lock(Loc, std::io::Error),
    #[error("Unable to open `{0}`: {1}")]
    Open(Loc, std::io::Error),
    #[error("Unable to read `{0}`: {1}")]
    Read(Loc, std::io::Error),
    #[error("Unable to read directory `{0}`: {1}")]
    ReadDir(Loc, std::io::Error),
    #[error("Unable to read pipe: {0}")]
    ReadPipe(std::io::Error),
    #[error("Unable to remove `{0}`: {1}")]
    Remove(Loc, std::io::Error),
    #[error("Unable to rename `{0}` to `{1}`: {2}")]
    Rename(Loc, Loc, std::io::Error),
    #[error("Unable to seek within `{0}`: {1}")]
    Seek(Loc, std::io::Error),
    #[error("`{0}` exceeds maximum expected size of {1} bytes")]
    FileTooLarge(Loc, usize),
    #[error("Unable to get metadata for file {0}: {1}")]
    Stat(Utf8PathBuf, std::io::Error),
    #[error("Unable to truncate `{0}`: {1}")]
    Truncate(Loc, std::io::Error),
    #[error("Unable to write to `{0}`: {1}")]
    Write(Loc, std::io::Error),

    // Signature errors
    #[error("Unable to sign `{0}`: {1}")]
    CouldNotSign(Loc, String),
    #[error("Repository index `{0}` is older than local instance; possible replay attack")]
    IndexTimestampOld(Loc),
    #[error("Signature for `{0}` is corrupt")]
    SigCorrupt(Loc),
    #[error("No configured public key verifies `{0}`")]
    SigInvalid(Loc),
    #[error("No public keys are configured for verifying `{0}`")]
    NoPublicKeys(Loc),
    #[error("`{0}` is not signed")]
    SigMissing(Loc),

    // Key loading errors
    #[error("Unable to load public key `{0}`: {1}")]
    LoadPublicKey(Utf8PathBuf, String),
    #[error("Unable to load private key `{0}`: {1}")]
    LoadSecretKey(Utf8PathBuf, String),
    #[error("Unrecognized public key format in `{0}`")]
    UnrecognizedKeyFormat(Utf8PathBuf),

    // Subprocess
    #[error("Network utility `{0}` exited with non-zero status {1}")]
    NetUtilNonZero(String, i32),
    #[error("Unable to drop privileges for shell script `{0}`: {1}")]
    DropPrivileges(Loc, std::io::Error),
    #[error("Unable to run shell script `{0}`: {1}")]
    ShellExec(Loc, std::io::Error),
    #[error("Message from script at `{0}` was corrupt: {1}")]
    ShellMsgCorrupt(Loc, &'static str),
    #[error("Shell script at `{0}` exited with non-zero status {1}")]
    ShellNonZero(Loc, i32),
    #[error("Unable to wait for shell script `{0}`: {1}")]
    ShellWait(Loc, std::io::Error),
    #[error("Unable to wait for shell script `{0}`: {1:#?}")]
    ShellWaitStatus(Loc, nix::sys::wait::WaitStatus),

    // General parsing errors
    #[error("{0} does not appear to be a valid {1}")]
    InvalidMagicNumber(Loc, FileType),
    #[error("{0} contains duplicate instances of '{1}' field")]
    FieldDuplicated(Loc, String),
    #[error("{0} lacks populated {1} field")]
    FieldEmpty(Loc, Field),
    #[error("{0} contains field {1} which contains disallowed {2} character")]
    FieldIllegalChar(Loc, Field, String),
    #[error("{0} contains invalid {1} field: {2}")]
    FieldInvalid(Loc, Field, String),
    #[error("{0} is missing field {1}")]
    FieldMissing(Loc, Field),
    #[error("{0} contains unexpected data")]
    UnexpectedData(Loc),

    // Source/checksum errors
    #[error("{0} has {1} source(s) but {2} checksum(s)")]
    SrcChecksumCountMismatch(Loc, usize, usize),

    // Encoding/decoding
    #[error("Unable to base64 decode within `{0}`: {1}")]
    Base64Decode(Loc, base64::DecodeError),
    #[error("Unable to build tarball from directory `{0}`: {1}")]
    BuildTarball(Loc, std::io::Error),
    #[error("Unable to compress contents at `{0}`: {1}")]
    Compress(Loc, std::io::Error),
    #[error("Unable to decompress contents at `{0}`: {1}")]
    Decompress(Loc, std::io::Error),
    #[error("`{0}` cannot be decoded to a valid UTF-8 string")]
    InvalidUnderscoreEncoding(String),
    #[error("Unable to parse tarball `{0}`: {1}")]
    ParseTarball(Loc, std::io::Error),
    #[error("Unable to unpack `{0}`: {1}")]
    UnpackTarball(Loc, std::io::Error),

    // File path errors
    #[error("Filename stem is not a recognized architecture: {0}")]
    FilenameStemArch(Loc),
    #[error("Path lacks filename: {0}")]
    PathLacksFileName(Loc),
    #[error("Path contains `..` component: {0}")]
    PathTraversal(Loc),

    // Networking
    #[error("No network utility found in $PATH")]
    NoNetUtilInPath,
    #[error("Unable to run network utility `{0}`: {1}")]
    NetUtilError(String, std::io::Error),

    // Config parsing errors
    #[error("{0}:{1} has an invalid {2}: {3}")]
    BptConfInvalidLine(String, usize, &'static str, String),

    // Miscellaneous
    #[error("Unable to run `brl which {0}`: returned non-utf8 output")]
    BrlWhichNonUtf8(Loc),
    #[error("Confirmation prompt denied")]
    ConfirmDenied,
    #[error("Unable to determine time since UNIX epoch: {0}")]
    GetTime(std::time::SystemTimeError),
    #[error("Unable to run `brl which {0}`: {1}")]
    RunBrlWhich(Loc, std::io::Error),
    #[error("When building `{0}` source `{1}` did not pass checksum")]
    SrcChecksumFailed(Loc, String),
}

/// An error lacking location context.  For example, if it's a Write error, which file are we
/// writing to?
///
/// Convert to `Err` with `.loc()`.
///
/// Note some errors here may not need location context, but are included so that a function which
/// returns those that should be AnonLocErr can also include it.  For these variants, the `.loc()`
/// methods are no-ops.
#[derive(Debug)]
pub enum AnonLocErr {
    // CLI/command errors
    MakeBinsMissingInPath(String),

    // I/O errors
    ClosePipe(std::io::Error),
    CreateAnon(std::io::Error),
    CreatePipe(std::io::Error),
    Chown(std::io::Error),
    Dup(nix::errno::Errno),
    Fcntl(std::io::Error),
    FlushStdout(std::io::Error),
    Fork(std::io::Error),
    Lock(std::io::Error),
    Open(std::io::Error),
    Read(std::io::Error),
    ReadPipe(std::io::Error),
    Seek(std::io::Error),
    Stat(std::io::Error),
    Truncate(std::io::Error),
    Write(std::io::Error),
    FileTooLarge(usize),

    // Signature errors
    CouldNotSign(String),
    SigCorrupt,
    SigInvalid,
    NoPublicKeys,
    SigMissing,

    // Subprocess
    DropPrivileges(std::io::Error),
    ShellExec(std::io::Error),
    ShellMsgCorrupt(&'static str),
    ShellNonZero(i32),
    ShellWait(std::io::Error),
    ShellWaitStatus(nix::sys::wait::WaitStatus),

    // General parsing errors
    InvalidMagicNumber(FileType),
    FieldDuplicated(String),
    FieldEmpty(Field),
    FieldIllegalChar(Field, String),
    FieldInvalid(Field, String),
    FieldMissing(Field),
    UnexpectedData,

    // Encoding/decoding
    Base64Decode(base64::DecodeError),
    BuildTarball(std::io::Error),
    Compress(std::io::Error),
    Decompress(std::io::Error),
    ParseTarball(std::io::Error),

    // Source/checksum errors
    SrcChecksumCountMismatch(usize, usize),

    // Config parsing errors
    BptConfInvalidLine(usize, &'static str, String),

    // Miscellaneous
    GetTime(std::time::SystemTimeError),
}

/// Expected command return value.
///
/// Ok() branch returns the successful command result, e.g. "Installed all 7 packages".
/// Err() branch returns top-level, user-facing error.
pub type CommandResult = Result<String, Err>;

/// An error lacking field context.  For example, if there's an illegal character in a field, which
/// field has that error?
///
/// Convert to `Err` with `.field()`.
#[derive(Debug)]
pub enum AnonFieldErr {
    IllegalChar(String),
}

impl AnonLocErr {
    /// Add location context to an AnonLocErr
    pub fn loc<T: std::fmt::Display>(self, loc: T) -> Err {
        let l = loc.to_string();
        match self {
            AnonLocErr::Base64Decode(e) => Err::Base64Decode(l, e),
            AnonLocErr::BptConfInvalidLine(n, t, e) => Err::BptConfInvalidLine(l, n, t, e),
            AnonLocErr::BuildTarball(e) => Err::BuildTarball(l, e),
            AnonLocErr::Chown(e) => Err::Chown(l, e),
            AnonLocErr::ClosePipe(e) => Err::ClosePipe(e),
            AnonLocErr::Compress(e) => Err::Compress(l, e),
            AnonLocErr::CouldNotSign(e) => Err::CouldNotSign(l, e),
            AnonLocErr::CreateAnon(e) => Err::CreateAnon(l, e),
            AnonLocErr::CreatePipe(e) => Err::CreatePipe(e),
            AnonLocErr::Decompress(e) => Err::Decompress(l, e),
            AnonLocErr::Dup(e) => Err::Dup(e),
            AnonLocErr::DropPrivileges(e) => Err::DropPrivileges(l, e),
            AnonLocErr::Fcntl(e) => Err::Fcntl(l, e),
            AnonLocErr::FieldDuplicated(f) => Err::FieldDuplicated(l, f),
            AnonLocErr::FieldEmpty(f) => Err::FieldEmpty(l, f),
            AnonLocErr::FieldIllegalChar(f, e) => Err::FieldIllegalChar(l, f, e),
            AnonLocErr::FieldInvalid(f, e) => Err::FieldInvalid(l, f, e),
            AnonLocErr::FieldMissing(f) => Err::FieldMissing(l, f),
            AnonLocErr::FileTooLarge(max) => Err::FileTooLarge(l, max),
            AnonLocErr::FlushStdout(e) => Err::FlushStdout(e),
            AnonLocErr::Fork(e) => Err::Fork(e),
            AnonLocErr::GetTime(e) => Err::GetTime(e),
            AnonLocErr::InvalidMagicNumber(e) => Err::InvalidMagicNumber(l, e),
            AnonLocErr::Lock(e) => Err::Lock(l, e),
            AnonLocErr::MakeBinsMissingInPath(s) => Err::MakeBinsMissingInPath(l, s),
            AnonLocErr::NoPublicKeys => Err::NoPublicKeys(l),
            AnonLocErr::Open(e) => Err::Open(l, e),
            AnonLocErr::ParseTarball(e) => Err::ParseTarball(l, e),
            AnonLocErr::Read(e) => Err::Read(l, e),
            AnonLocErr::ReadPipe(e) => Err::ReadPipe(e),
            AnonLocErr::Seek(e) => Err::Seek(l, e),
            AnonLocErr::ShellExec(e) => Err::ShellExec(l, e),
            AnonLocErr::ShellMsgCorrupt(e) => Err::ShellMsgCorrupt(l, e),
            AnonLocErr::ShellNonZero(e) => Err::ShellNonZero(l, e),
            AnonLocErr::ShellWait(e) => Err::ShellWait(l, e),
            AnonLocErr::ShellWaitStatus(e) => Err::ShellWaitStatus(l, e),
            AnonLocErr::SigCorrupt => Err::SigCorrupt(l),
            AnonLocErr::SigInvalid => Err::SigInvalid(l),
            AnonLocErr::SigMissing => Err::SigMissing(l),
            AnonLocErr::SrcChecksumCountMismatch(s, c) => Err::SrcChecksumCountMismatch(l, s, c),
            AnonLocErr::Stat(e) => Err::Stat(Utf8PathBuf::from(l), e),
            AnonLocErr::Truncate(e) => Err::Truncate(l, e),
            AnonLocErr::UnexpectedData => Err::UnexpectedData(l),
            AnonLocErr::Write(e) => Err::Write(l, e),
        }
    }
}

pub trait WithLocation<O> {
    fn loc<L: std::fmt::Display>(self, loc: L) -> Result<O, Err>;
}

impl<O> WithLocation<O> for Result<O, AnonLocErr> {
    fn loc<L: std::fmt::Display>(self, loc: L) -> Result<O, Err> {
        self.map_err(|e| e.loc(loc))
    }
}

impl AnonFieldErr {
    pub fn field(self, f: Field) -> AnonLocErr {
        match self {
            AnonFieldErr::IllegalChar(e) => AnonLocErr::FieldIllegalChar(f, e),
        }
    }
}

impl From<std::str::Utf8Error> for AnonFieldErr {
    fn from(_: std::str::Utf8Error) -> Self {
        AnonFieldErr::IllegalChar("non-utf8".to_owned())
    }
}

/// Add the location to errors which lack it
pub trait WithField<O> {
    fn field(self, field: Field) -> Result<O, AnonLocErr>;
}

impl<O> WithField<O> for Result<O, AnonFieldErr> {
    fn field(self, field: Field) -> Result<O, AnonLocErr> {
        self.map_err(|e| e.field(field))
    }
}

impl<O> WithField<O> for Result<O, std::str::Utf8Error> {
    fn field(self, field: Field) -> Result<O, AnonLocErr> {
        self.map_err(|e| AnonFieldErr::from(e).field(field))
    }
}

// Most non-I/O exit codes follow sysexits.h conventions.
// I/O errors forward the OS errno directly via raw_os_error().
impl Err {
    pub fn exit_code(&self) -> i32 {
        match self {
            // I/O errors: forward OS errno when available
            Err::ClosePipe(e)
            | Err::CreateAnon(_, e)
            | Err::CreateDir(_, e)
            | Err::CreateFile(_, e)
            | Err::CreateMemFd(e)
            | Err::CreatePipe(e)
            | Err::Chown(_, e)
            | Err::Compress(_, e)
            | Err::Decompress(_, e)
            | Err::DropPrivileges(_, e)
            | Err::Fcntl(_, e)
            | Err::Fork(e)
            | Err::FlushStdout(e)
            | Err::GetXattr(_, e)
            | Err::Link(_, e)
            | Err::Lock(_, e)
            | Err::Open(_, e)
            | Err::ParseTarball(_, e)
            | Err::Read(_, e)
            | Err::ReadDir(_, e)
            | Err::ReadPipe(e)
            | Err::Remove(_, e)
            | Err::Rename(_, _, e)
            | Err::RunBrlWhich(_, e)
            | Err::Seek(_, e)
            | Err::ShellExec(_, e)
            | Err::ShellWait(_, e)
            | Err::Stat(_, e)
            | Err::Truncate(_, e)
            | Err::BuildTarball(_, e)
            | Err::UnpackTarball(_, e)
            | Err::Write(_, e) => e.raw_os_error().unwrap_or(1),

            Err::Dup(e) => *e as i32,

            // Subprocess
            Err::NetUtilNonZero(_, rv) => *rv,
            Err::ShellNonZero(_, rv) => *rv,
            Err::ShellWaitStatus(..) => 1,

            // Signature errors
            Err::CouldNotSign(..)
            | Err::IndexTimestampOld(..)
            | Err::NoPublicKeys(..)
            | Err::SigCorrupt(..)
            | Err::SigInvalid(..)
            | Err::SigMissing(..) => 62,

            // Key loading errors
            Err::LoadPublicKey(..) | Err::LoadSecretKey(..) | Err::UnrecognizedKeyFormat(..) => 63,

            // CLI errors (EX_USAGE)
            Err::BuildBbuild
            | Err::BuildDependencyCycle(..)
            | Err::BuildNative
            | Err::BuildOutputExists(..)
            | Err::BuildPkg(..)
            | Err::InputFieldInvalid(..)
            | Err::InvalidRegex(..)
            | Err::MakeRepoNoBbuilds(..)
            | Err::MakeBinsMissingInPath(..)
            | Err::NoDefaultArchForBbuild(..)
            | Err::UnableToLocateInstalledPkg(..)
            | Err::RemovePkgNotExplicit(..)
            | Err::DowngradeDependencyPkg(..)
            | Err::RuntimeDependencyCycle(..)
            | Err::UnableToLocateAvailablePkg(..)
            | Err::UnableToLocateDependency(..)
            | Err::UnableToLocateRepositoryPkg(..) => 64,

            // Parsing errors (EX_DATAERR)
            Err::FileTooLarge(..)
            | Err::DependArchMissing(..)
            | Err::InvalidMagicNumber(..)
            | Err::FieldDuplicated(..)
            | Err::FieldEmpty(..)
            | Err::FieldIllegalChar(..)
            | Err::FieldInvalid(..)
            | Err::FieldMissing(..)
            | Err::ShellMsgCorrupt(..)
            | Err::SrcChecksumCountMismatch(..)
            | Err::UnexpectedData(..) => 65,

            // Encoding/decoding errors (EX_NOINPUT)
            Err::Base64Decode(..) | Err::InvalidUnderscoreEncoding(..) => 66,

            // File path errors (EX_NOUSER)
            Err::FilenameStemArch(..) | Err::PathLacksFileName(..) | Err::PathTraversal(..) => 67,

            // Networking
            Err::NoNetUtilInPath | Err::NetUtilError(..) => 68,

            // Location
            Err::InvalidBptPath(..)
            | Err::InvalidBptUrl(..)
            | Err::InvalidBptPathUrlRepo(..)
            | Err::InvalidBbuildPath(..)
            | Err::InvalidBbuildUrl(..)
            | Err::InvalidBbuildPathUrlRepo(..)
            | Err::InvalidIdxPath(..)
            | Err::InvalidIdxPathUrl(..)
            | Err::InvalidIdxUrl(..)
            | Err::InvalidPkgPath(..)
            | Err::InvalidPkgPathUrlRepo(..)
            | Err::InvalidPkgUrl(..)
            | Err::InvalidUrl(..) => 69,

            // Miscellaneous
            Err::BptConfInvalidLine(..)
            | Err::BrlWhichNonUtf8(..)
            | Err::CheckFailed(..)
            | Err::ConfirmDenied
            | Err::GetTime(..)
            | Err::InstallConflict(..)
            | Err::SrcChecksumFailed(..) => 70,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::Err;

    #[test]
    fn build_tarball_uses_io_exit_code() {
        let err = Err::BuildTarball(
            "test-dir".to_string(),
            std::io::Error::from_raw_os_error(28),
        );
        assert_eq!(err.exit_code(), 28);
    }

    #[test]
    fn unpack_tarball_uses_io_exit_code() {
        let err = Err::UnpackTarball(
            "test-pkg".to_string(),
            std::io::Error::from_raw_os_error(13),
        );
        assert_eq!(err.exit_code(), 13);
    }
}
