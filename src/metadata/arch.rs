use crate::{
    color::*, constant::*, error::*, make_display_color, make_field, marshalling::*, metadata::*,
};
use std::{borrow::Cow, ffi::OsStr, os::unix::prelude::OsStrExt, str::FromStr};

/// Generate enum with an associated:
///
/// - `FromStr` implementation.  Used by CLAP.  (Most of the code base prefers `try_from::<&str>`)
/// - `as_str()`
macro_rules! make_arch {
    ($name:ident {
        $( $variant:ident, )*
    }) => {
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $( $variant, )*
        }
        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $( $name::$variant => stringify!($variant), )*
                }
            }
        }
        impl std::str::FromStr for $name {
            type Err = Err;
            fn from_str(input: &str) -> Result<Self, Self::Err> {
                match input {
                    "host" => Ok(Arch::host()),
                    $( stringify!($variant) => Ok($name::$variant), )*
                    _ => Err(Err::InputFieldInvalid(Self::NAME, input.to_string())),
                }
            }
        }
    }
}

// Instruction Set Architecture of a package.
make_arch!(Arch {
    bbuild, // Package build definition.
    native, // Non-portable optimizations for the local machine.
    noarch, // Architecture agnostic, e.g. shell scripts.
    aarch64,
    armv7hl,
    armv7l,
    i586,
    i686,
    loongarch64,
    mips,
    mips64,
    mips64el,
    mipsel,
    powerpc,
    powerpc64,
    powerpc64le,
    riscv64gc,
    s390x,
    x86_64,
});

make_field!(Arch, PkgKey);

impl Arch {
    pub fn host() -> Self {
        HOST_ARCH
    }
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

make_display_color!(Arch, |s, f| {
    write!(f, "{}{}{}", Color::Arch, s, Color::Default)
});

impl AsBytes for Arch {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.as_str().as_bytes())
    }
}

impl std::convert::TryFrom<&str> for Arch {
    type Error = AnonLocErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // FromStr is only used by the CLI and thus this needs different context information.
        Self::from_str(value).map_err(|_| AnonLocErr::FieldInvalid(Self::NAME, value.to_string()))
    }
}

impl FromFieldStr for Arch {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        Self::try_from(value.as_str())
    }
}

impl std::convert::TryFrom<&[u8]> for Arch {
    type Error = AnonLocErr;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let str = std::str::from_utf8(value).field(Self::NAME)?;
        Self::try_from(str)
    }
}

impl std::convert::TryFrom<&OsStr> for Arch {
    type Error = AnonLocErr;

    fn try_from(value: &OsStr) -> Result<Self, Self::Error> {
        value.as_bytes().try_into()
    }
}
