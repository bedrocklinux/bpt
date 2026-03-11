use crate::{
    error::*, io::reject_unsafe_path, make_display_color, make_field, marshalling::*, metadata::*,
};
use std::borrow::Cow;

/// List of user-customizable files which should be retained during upgrade or removal of a
/// package if they have been modified.
#[derive(Clone, Debug)]
pub struct Backup(Vec<Filepath>);

make_field!(Backup, PkgKey);

impl Backup {
    #[cfg(test)]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[Filepath] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Filepath> {
        self.0.iter()
    }
}

impl std::fmt::Display for Backup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for entry in &self.0 {
            if first {
                first = false;
            } else {
                write!(f, " ")?;
            }
            write!(f, "{}", entry)?;
        }
        Ok(())
    }
}

make_display_color!(Backup, |s, f| {
    let mut first = true;
    for entry in &s.0 {
        if first {
            first = false;
        } else {
            write!(f, " ")?;
        }
        write!(f, "{}", entry.color())?;
    }
    Ok(())
});

impl FromFieldStr for Backup {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        let mut entries = Vec::new();
        for entry in value.split_whitespace() {
            reject_unsafe_path(entry.as_str(), Self::NAME)?;
            entries.push(Filepath::from_field_str(entry)?);
        }
        Ok(Self(entries))
    }
}

impl AsBytes for Backup {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        match self.as_slice() {
            [] => Cow::Borrowed(b""),
            [single] => single.as_bytes(),
            [first, rest @ ..] => {
                let mut bytes = Vec::from(first.as_bytes().as_ref());
                for entry in rest {
                    bytes.push(b' ');
                    bytes.extend_from_slice(entry.as_bytes().as_ref());
                }
                Cow::Owned(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Backup, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Backup::NAME))
            .and_then(Backup::from_field_str)
    }

    #[test]
    fn roundtrip() {
        let backup = parse("etc/fakeblock.conf var/lib/fakeblock/state").unwrap();
        assert_eq!(
            std::str::from_utf8(&backup.as_bytes()).unwrap(),
            "etc/fakeblock.conf var/lib/fakeblock/state"
        );
    }

    #[test]
    fn empty_is_empty_list() {
        let backup = parse("").unwrap();
        assert!(backup.as_slice().is_empty());
    }

    #[test]
    fn absolute_path_rejected() {
        for path in &[
            "/etc/fakeblock.conf",
            "etc/fakeblock.conf /var/lib/fakeblock/state",
        ] {
            let err = parse(path).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains("absolute"),
                "expected absolute path error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn dotdot_rejected() {
        for path in &["../etc/fakeblock.conf", "etc/../fakeblock.conf"] {
            let err = parse(path).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains(".."),
                "expected `..` path error for `{path}`, got: {msg}"
            );
        }
    }
}
