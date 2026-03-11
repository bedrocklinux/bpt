use crate::{constant::*, error::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{ffi::OsStr, os::unix::prelude::OsStrExt};

/// Field string.
///
/// All fields shall be valid UTF-8 and may not contain null bytes (as fields are stored in
/// null-separated blocks).  This type enforces these invariants, and thus many field types are
/// built around it to inherit its enforcement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldStr(String);

impl FieldStr {
    pub fn empty() -> Self {
        Self(String::new())
    }

    pub fn split_whitespace(&self) -> impl Iterator<Item = Self> + '_ {
        self.0
            .split(WHITESPACE_CHARS)
            .filter(|s| !s.is_empty())
            .map(|s| Self(s.to_owned()))
    }

    pub fn split_once(&self, delimiter: &str) -> Option<(FieldStr, FieldStr)> {
        self.0
            .split_once(delimiter)
            .map(|(a, b)| (Self(a.to_owned()), Self(b.to_owned())))
    }

    pub fn split_at(&self, mid: usize) -> (FieldStr, FieldStr) {
        let (a, b) = self.0.split_at(mid);
        (Self(a.to_owned()), Self(b.to_owned()))
    }

    pub fn push(&mut self, c: char) -> Result<(), AnonFieldErr> {
        if c == '\0' {
            Err(AnonFieldErr::IllegalChar("null".to_owned()))
        } else {
            self.0.push(c);
            Ok(())
        }
    }

    pub fn push_str(&mut self, s: &str) -> Result<(), AnonFieldErr> {
        if s.contains('\0') {
            Err(AnonFieldErr::IllegalChar("null".to_owned()))
        } else {
            self.0.push_str(s);
            Ok(())
        }
    }

    pub fn push_fieldstr(&mut self, other: &Self) {
        self.0.push_str(&other.0);
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn into_pathbuf(self) -> Utf8PathBuf {
        Utf8PathBuf::from(self.0)
    }
}

impl std::ops::Deref for FieldStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for FieldStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<FieldStr> for String {
    fn from(s: FieldStr) -> String {
        s.0
    }
}

impl TryFrom<String> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.contains('\0') {
            Err(AnonFieldErr::IllegalChar("null".to_owned()))
        } else {
            Ok(Self(value))
        }
    }
}

impl TryFrom<&str> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_string())
    }
}

impl TryFrom<&[u8]> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from(std::str::from_utf8(value).map_err(AnonFieldErr::from)?)
    }
}

impl TryFrom<&OsStr> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: &OsStr) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl TryFrom<Utf8PathBuf> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: Utf8PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.into_string())
    }
}

impl TryFrom<&Utf8Path> for FieldStr {
    type Error = AnonFieldErr;

    fn try_from(value: &Utf8Path) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let empty = FieldStr::empty();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_split_whitespace() {
        let s = FieldStr("a b\tc\nd".into());
        let words: Vec<_> = s.split_whitespace().collect();
        assert_eq!(
            words,
            vec![
                FieldStr("a".into()),
                FieldStr("b".into()),
                FieldStr("c".into()),
                FieldStr("d".into())
            ]
        );
    }

    #[test]
    fn test_split_once() {
        let s = FieldStr("key:value".into());
        let (key, value) = s.split_once(":").unwrap();
        assert_eq!(key, FieldStr("key".into()));
        assert_eq!(value, FieldStr("value".into()));
    }

    #[test]
    fn test_split_at() {
        let s = FieldStr("hello world".into());
        let (hello, world) = s.split_at(6);
        assert_eq!(hello, FieldStr("hello ".into()));
        assert_eq!(world, FieldStr("world".into()));
    }

    #[test]
    fn test_push() {
        let mut s = FieldStr::empty();
        s.push('a').unwrap();
        assert_eq!(s, FieldStr("a".into()));
    }

    #[test]
    fn test_push_str() {
        let mut s = FieldStr::empty();
        s.push_str("hello").unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_as_bytes() {
        let s = FieldStr("hello".into());
        assert_eq!(s.as_bytes(), b"hello");
    }

    #[test]
    fn test_as_str() {
        let s = FieldStr("hello".into());
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_into_string() {
        let s = FieldStr("hello".into());
        let string: String = s.into_string();
        assert_eq!(string, "hello");
    }

    #[test]
    fn test_from_string() {
        let s = FieldStr::try_from("hello".to_string()).unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_from_str() {
        let s = FieldStr::try_from("hello").unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_from_bytes() {
        let s = FieldStr::try_from(b"hello".as_ref()).unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_from_os_str() {
        let s = FieldStr::try_from(OsStr::new("hello")).unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_from_utf8_path_buf() {
        let s = FieldStr::try_from(Utf8PathBuf::from("hello")).unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_from_utf8_path() {
        let s = FieldStr::try_from(Utf8Path::new("hello")).unwrap();
        assert_eq!(s, FieldStr("hello".into()));
    }

    #[test]
    fn test_illegal_char() {
        assert!(FieldStr::try_from("hello\0world").is_err());
        let mut s = FieldStr::empty();
        assert!(s.push('\0').is_err());
        assert!(s.push_str("hello\0world").is_err());
    }
}
