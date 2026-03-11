use crate::{constant::*, error::*, make_display_color, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// A binary expected to be in the `$PATH` at build time
/// For example, a compiler.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MakeBin {
    CoreGroup,
    AutotoolsGroup,
    Single(FieldStr),
}

make_field!(MakeBin, PkgKey);

impl<'a, 'b> MakeBin {
    pub fn as_str(&self) -> &str {
        match self {
            MakeBin::CoreGroup => "@core",
            MakeBin::AutotoolsGroup => "@autotools",
            MakeBin::Single(s) => s.as_str(),
        }
    }

    pub fn expanded(&'a self, array: &'b mut [&'a str; 1]) -> &'b [&'a str] {
        match self {
            MakeBin::CoreGroup => CORE_MAKEBINS,
            MakeBin::AutotoolsGroup => AUTOTOOLS_MAKEBINS,
            MakeBin::Single(s) => {
                array[0] = s.as_str();
                array.as_slice()
            }
        }
    }
}

impl std::fmt::Display for MakeBin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

make_display_color!(MakeBin, |s, f| { write!(f, "{}", s.as_str()) });

impl FromFieldStr for MakeBin {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            return Err(AnonLocErr::FieldEmpty(Self::NAME));
        }

        if let Some(c) = value
            .as_str()
            .chars()
            .find(|&c| WHITESPACE_CHARS.contains(&c) || c == '/')
        {
            return Err(AnonLocErr::FieldIllegalChar(Self::NAME, c.to_string()));
        }

        match value.as_str() {
            "@core" => Ok(MakeBin::CoreGroup),
            "@autotools" => Ok(MakeBin::AutotoolsGroup),
            v if v.starts_with('@') => Err(AnonLocErr::FieldInvalid(
                Self::NAME,
                format!("Unrecognized group alias `{}`", value.as_str()),
            )),
            _ => Ok(MakeBin::Single(value)),
        }
    }
}

impl AsBytes for MakeBin {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.as_str().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<MakeBin, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(MakeBin::NAME))
            .and_then(MakeBin::from_field_str)
    }

    #[test]
    fn valid_makebins_parse() {
        for makebin in [
            "make",
            "pkg-config",
            "g++",
            "python3.12",
            "x86_64-linux-gnu-gcc",
        ] {
            let parsed = parse(makebin).unwrap();
            assert_eq!(parsed.as_str(), makebin);
            assert_eq!(parsed.to_string(), makebin);
            assert_eq!(parsed.as_bytes().as_ref(), makebin.as_bytes());
        }
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            parse("").unwrap_err(),
            AnonLocErr::FieldEmpty(MakeBin::NAME)
        ));
    }

    #[test]
    fn slash_rejected() {
        assert!(matches!(
            parse("usr/bin/make").unwrap_err(),
            AnonLocErr::FieldIllegalChar(MakeBin::NAME, _)
        ));
    }

    #[test]
    fn whitespace_rejected() {
        for makebin in ["gnu make", "gnu\tmake", "gnu\nmake"] {
            assert!(matches!(
                parse(makebin).unwrap_err(),
                AnonLocErr::FieldIllegalChar(MakeBin::NAME, _)
            ));
        }
    }

    #[test]
    fn recognized_group_aliases_parse() {
        assert!(matches!(parse("@core").unwrap(), MakeBin::CoreGroup));
        assert!(matches!(
            parse("@autotools").unwrap(),
            MakeBin::AutotoolsGroup
        ));
    }

    #[test]
    fn unrecognized_group_alias_rejected() {
        assert!(matches!(
            parse("@bogus").unwrap_err(),
            AnonLocErr::FieldInvalid(MakeBin::NAME, _)
        ));
    }
}
