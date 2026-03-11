use crate::{
    color::*, constant::*, error::*, make_display_color, make_field, marshalling::*, metadata::*,
};
use std::borrow::Cow;

/// Version of a package
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PkgVer {
    pub epoch: u32,
    pub semver: semver::Version,
    pub pkgrel: u32,
}

make_field!(PkgVer, PkgKey);

make_display_color!(PkgVer, |s, f| {
    write!(f, "{}{}{}", Color::PkgVer, s, Color::Default)
});

impl std::fmt::Display for PkgVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.epoch != 0 {
            write!(f, "e{}-", self.epoch)?;
        }
        write!(f, "{}", self.semver)?;
        if self.pkgrel != 0 {
            write!(f, "-r{}", self.pkgrel)?;
        }
        Ok(())
    }
}

impl FromFieldStr for PkgVer {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        let value = value.as_str();
        if value.is_empty() {
            return Err(AnonLocErr::FieldEmpty(Self::NAME));
        }

        if let Some(c) = value.chars().next()
            && is_pkgver_start_char_disallowed(c)
        {
            return Err(AnonLocErr::FieldIllegalChar(
                Self::NAME,
                format!("'{c}' as starting character"),
            ));
        }

        if let Some(c) = value.chars().find(|&c| !is_pkgver_char(c)) {
            return Err(AnonLocErr::FieldIllegalChar(Self::NAME, c.to_string()));
        }

        // - Epoch may precede the semver, preceded by an `e` separated by a hyphen.  If it is
        // missing, treat it as 0.
        // - Pkgrel may follow the semver, preceded by an 'r' and separated by a hyphen.  If it is
        // missing, treat it as 0.
        // - Colon is traditionally used for the epoch separator, but in bpt it's reserved for the
        // arch separator.  Hence epoch uses an 'e' prefix.

        let (epoch, rest) = match value.strip_prefix('e').and_then(|v| v.split_once('-')) {
            None => (None, value),
            Some((epoch, rest)) if epoch.chars().all(|c| c.is_ascii_digit()) => (Some(epoch), rest),
            Some(_) => (None, value),
        };

        let (semver, pkgrel) = match rest.rsplit_once("-r") {
            None => (rest, None),
            Some((semver, pkgrel)) if pkgrel.chars().all(|c| c.is_ascii_digit()) => {
                (semver, Some(pkgrel))
            }
            Some(_) => (rest, None),
        };

        let epoch = epoch
            .map(|s| s.parse())
            .transpose()
            .map_err(|_| AnonLocErr::FieldInvalid(Self::NAME, value.to_owned()))?
            .unwrap_or(0);

        let semver = semver::Version::parse(semver)
            .map_err(|_| AnonLocErr::FieldInvalid(Self::NAME, value.to_owned()))?;

        let pkgrel = pkgrel
            .map(|s| s.parse())
            .transpose()
            .map_err(|_| AnonLocErr::FieldInvalid(Self::NAME, value.to_owned()))?
            .unwrap_or(0);

        Ok(Self {
            epoch,
            semver,
            pkgrel,
        })
    }
}

impl AsBytes for PkgVer {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.to_string().into_bytes())
    }
}

impl TryFrom<&str> for PkgVer {
    type Error = AnonLocErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        FieldStr::try_from(value)
            .map_err(|e| e.field(Self::NAME))
            .and_then(Self::from_field_str)
    }
}

impl std::str::FromStr for PkgVer {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s).map_err(|_| Err::InputFieldInvalid(Self::NAME, s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_semver_conversion() {
        let version_str = "1.0.0";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_semver_conversion_with_pre_release() {
        let version_str = "1.0.0-alpha";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_semver_conversion_with_build_metadata() {
        let version_str = "1.0.0+build.1";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_semver_conversion_with_pre_release_and_build_metadata() {
        let version_str = "1.0.0-alpha+build.1";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_invalid_semver_conversion() {
        let version_str = "1.0";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        assert!(
            PkgVer::from_field_str(version_fstr).is_err(),
            "Parsed an invalid SemVer string"
        );
    }

    #[test]
    fn test_invalid_chars_rejected() {
        for bad in ["1.2.3:", "1.2.3@", "1.2.3/"] {
            let err = PkgVer::try_from(bad).expect_err("invalid pkgver unexpectedly parsed");
            assert!(
                matches!(err, AnonLocErr::FieldIllegalChar(PkgVer::NAME, _)),
                "expected FieldIllegalChar for `{bad}`, got {err:?}"
            );
        }
    }

    #[test]
    fn test_disallowed_start_char_rejected() {
        for bad in ["=1.2.3", "<1.2.3", ">1.2.3", "^1.2.3", "~1.2.3"] {
            let err = PkgVer::try_from(bad).expect_err("invalid pkgver unexpectedly parsed");
            assert!(
                matches!(err, AnonLocErr::FieldIllegalChar(PkgVer::NAME, _)),
                "expected FieldIllegalChar for `{bad}`, got {err:?}"
            );
        }
    }

    #[test]
    fn test_malformed_epoch_and_pkgrel_rejected() {
        for bad in [
            "e1.2.3",
            "e-1.2.3",
            "eabc-1.2.3",
            "e1-1.2",
            "e1-1.2.3-r4294967296",
        ] {
            let err = PkgVer::try_from(bad).expect_err("malformed pkgver unexpectedly parsed");
            assert!(
                matches!(err, AnonLocErr::FieldInvalid(PkgVer::NAME, _)),
                "expected FieldInvalid for `{bad}`, got {err:?}"
            );
        }
    }

    #[test]
    fn test_display_normalizes_epoch_and_pkgrel_numbers() {
        let v = PkgVer::try_from("e01-1.2.3-r004").expect("failed to parse version");
        assert_eq!(v.epoch, 1);
        assert_eq!(v.pkgrel, 4);
        assert_eq!(v.to_string(), "e1-1.2.3-r4");
    }

    #[test]
    fn test_epoch() {
        let version_str = "e1-2.3.4";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.epoch, 1);
        assert_eq!(version.semver, semver::Version::new(2, 3, 4));
        assert_eq!(version.pkgrel, 0);
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_pkgrel() {
        let version_str = "1.2.3-r4";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.epoch, 0);
        assert_eq!(version.semver, semver::Version::new(1, 2, 3));
        assert_eq!(version.pkgrel, 4);
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_epoch_pkgrel() {
        let version_str = "e1-2.3.4-r5";
        let version_fstr = FieldStr::try_from(version_str).unwrap();
        let version = PkgVer::from_field_str(version_fstr).unwrap();
        assert_eq!(version.epoch, 1);
        assert_eq!(version.semver, semver::Version::new(2, 3, 4));
        assert_eq!(version.pkgrel, 5);
        assert_eq!(version.to_string(), version_str);
    }

    #[test]
    fn test_cmp() {
        // Versions are manually ordered from lowest to highest.  We can leverage the index
        // comparisons to validate the pkgver comparisons.
        let versions = &[
            "0.9.9",
            "0.9.9-r1",
            "0.9.9-r2",
            "1.0.0",
            "1.0.0-r1",
            "1.0.0-r2",
            "1.0.1",
            "1.0.1-r1",
            "1.0.1-r2",
            "e1-0.9.9",
            "e1-0.9.9-r1",
            "e1-0.9.9-r2",
            "e1-1.0.0",
            "e1-1.0.0-r1",
            "e1-1.0.0-r2",
            "e1-1.0.1",
            "e1-1.0.1-r1",
            "e1-1.0.1-r2",
            "e2-0.9.9",
            "e2-0.9.9-r1",
            "e2-0.9.9-r2",
            "e2-1.0.0",
            "e2-1.0.0-r1",
            "e2-1.0.0-r2",
            "e2-1.0.1",
            "e2-1.0.1-r1",
            "e2-1.0.1-r2",
        ];

        for (i, version_a) in versions.iter().enumerate() {
            let a_fstr = FieldStr::try_from(*version_a).unwrap();
            let a = PkgVer::from_field_str(a_fstr).unwrap();

            for (j, version_b) in versions.iter().enumerate() {
                let b_fstr = FieldStr::try_from(*version_b).unwrap();
                let b = PkgVer::from_field_str(b_fstr).unwrap();

                if i < j {
                    assert!(a != b);
                    assert!(a < b);
                    assert!(a <= b);
                    assert!(b > a);
                    assert!(b >= a);
                } else if i == j {
                    assert!(a == b);
                    assert!(a <= b);
                    assert!(a >= b);
                } else {
                    assert!(a != b);
                    assert!(a > b);
                    assert!(a >= b);
                    assert!(b < a);
                    assert!(b <= a);
                }
            }
        }
    }
}
