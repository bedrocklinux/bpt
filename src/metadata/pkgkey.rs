use crate::constant::*;

/// PkgInfo Field key.  See [crate::marshalling::Field::KEY] for more information.
///
/// Communication with build scripts includes keys within shell script strings.  Keys
/// must not contain shell-sensitive string-escaping characters.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum PkgKey {
    Arch = b'a',
    Backup = b'b',
    Depend = b'-', // Component of other fields; never directly serialized
    Depends = b'D',
    License = b'l',
    Filepath = b'_', // Component of other fields; never directly serialized
    MakeBin = b'M',  // Component of other fields; never directly serialized
    RepoPath = b'R',
    MakeArchs = b'A',
    MakeBins = b'B',
    MakeDepends = b'm',
    PkgDesc = b'd',
    PkgName = b'n',
    PkgVer = b'v',
    Homepage = b'h',
}

impl std::fmt::Display for PkgKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // String representation of keys may be embedded within shell strings, and thus must not
        // contain shell-string escaping characters.
        debug_assert!(!SHELL_SPECIAL_CHARS.contains(&self.as_char()));
        write!(f, "{}", self.as_char())
    }
}

impl PkgKey {
    pub const fn as_char(&self) -> char {
        *self as u8 as char
    }
}
