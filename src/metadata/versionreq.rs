use crate::{color::*, error::*, make_display_color, marshalling::*, metadata::*};

/// A version requirement for a package.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionReq {
    pub epoch: u32,
    pub semver: semver::VersionReq,
    pub pkgrel: u32,
}

make_display_color!(VersionReq, |s, f| {
    // Separate out op and semver body so we can squeeze epoch in between
    let Some(mut cmp) = s.semver.comparators.first().cloned() else {
        return Err(std::fmt::Error);
    };

    write!(f, "{}", Color::Glue)?;
    match cmp.op {
        semver::Op::Exact => write!(f, "=")?,
        semver::Op::Greater => write!(f, ">")?,
        semver::Op::GreaterEq => write!(f, ">=")?,
        semver::Op::Less => write!(f, "<")?,
        semver::Op::LessEq => write!(f, "<=")?,
        semver::Op::Tilde => write!(f, "~")?,
        semver::Op::Caret => write!(f, "^")?,
        _ => return Err(std::fmt::Error),
    };

    write!(f, "{}", Color::PkgVer)?;
    if s.epoch != 0 {
        write!(f, "e{}-", s.epoch)?;
    }

    cmp.op = semver::Op::Exact; // force expected opt so we can strip it
    write!(f, "{}", cmp.to_string().strip_prefix('=').unwrap())?;

    if s.pkgrel != 0 {
        write!(f, "-r{}", s.pkgrel)?;
    }
    write!(f, "{}", Color::Default)
});

impl std::fmt::Display for VersionReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Separate out op and semver body so we can squeeze epoch in between
        let Some(mut cmp) = self.semver.comparators.first().cloned() else {
            return Err(std::fmt::Error);
        };

        match cmp.op {
            semver::Op::Exact => write!(f, "=")?,
            semver::Op::Greater => write!(f, ">")?,
            semver::Op::GreaterEq => write!(f, ">=")?,
            semver::Op::Less => write!(f, "<")?,
            semver::Op::LessEq => write!(f, "<=")?,
            semver::Op::Tilde => write!(f, "~")?,
            semver::Op::Caret => write!(f, "^")?,
            _ => return Err(std::fmt::Error),
        };

        if self.epoch != 0 {
            write!(f, "e{}-", self.epoch)?;
        }

        cmp.op = semver::Op::Exact; // force expected opt so we can strip it
        write!(f, "{}", cmp.to_string().strip_prefix('=').unwrap())?;

        if self.pkgrel != 0 {
            write!(f, "-r{}", self.pkgrel)?;
        }

        Ok(())
    }
}

impl VersionReq {
    pub fn from_field_str(value: FieldStr) -> Result<Self, AnonFieldErr> {
        // <op>[e<epoch>-]<semver>[-r<pkgrel>]
        let value = value.as_str();

        // Extract op
        let (op, rest) = if let Some(rest) = value.strip_prefix(">=") {
            (">=", rest)
        } else if let Some(rest) = value.strip_prefix("<=") {
            ("<=", rest)
        } else if let Some(rest) = value.strip_prefix("==") {
            ("==", rest)
        } else if let Some(rest) = value.strip_prefix('=') {
            ("=", rest)
        } else if let Some(rest) = value.strip_prefix('>') {
            (">", rest)
        } else if let Some(rest) = value.strip_prefix('<') {
            ("<", rest)
        } else if let Some(rest) = value.strip_prefix('~') {
            ("~", rest)
        } else if let Some(rest) = value.strip_prefix('^') {
            ("^", rest)
        } else {
            return Err(AnonFieldErr::IllegalChar("non-semver op".to_owned()));
        };

        let (epoch, rest) = match rest.strip_prefix('e').and_then(|v| v.split_once('-')) {
            None => (None, rest),
            Some((epoch, rest)) if epoch.chars().all(|c| c.is_ascii_digit()) => (Some(epoch), rest),
            Some(_) => (None, rest),
        };

        let (semver, pkgrel) = match rest.rsplit_once("-r") {
            None => (rest, None),
            Some((semver, pkgrel)) if pkgrel.chars().all(|c| c.is_ascii_digit()) => {
                (semver, Some(pkgrel))
            }
            Some(_) => (rest, None),
        };

        // Re-attach op to semver
        let semver = [op, semver].concat();
        let semver = semver::VersionReq::parse(&semver)
            .map_err(|_| AnonFieldErr::IllegalChar("non-semver".to_owned()))?;

        Ok(Self {
            epoch: epoch
                .map(|s| s.parse())
                .transpose()
                .map_err(|_| AnonFieldErr::IllegalChar("non-epoch".to_owned()))?
                .unwrap_or(0),
            semver,
            pkgrel: pkgrel
                .map(|s| s.parse())
                .transpose()
                .map_err(|_| AnonFieldErr::IllegalChar("non-pkgrel".to_owned()))?
                .unwrap_or(0),
        })
    }

    pub fn provided_by(&self, pkgver: &PkgVer) -> bool {
        let epoch = pkgver.epoch;
        let semver = &pkgver.semver;
        let pkgrel = pkgver.pkgrel;

        // The semver package represents this concept as a list.  However, we only support a single
        // item here and use our own lists at one abstraction level higher in [Depends].
        let Some(cmp) = self.semver.comparators.first() else {
            return false;
        };

        // Create an equality check for the semver to leverage in pkgrel checks.
        let mut eqcmp = cmp.clone();
        eqcmp.op = semver::Op::Exact;
        let equal = semver::VersionReq {
            comparators: vec![eqcmp],
        };

        match cmp.op {
            semver::Op::Exact if epoch != self.epoch => false,
            semver::Op::Exact if !self.semver.matches(semver) => false,
            semver::Op::Exact => pkgrel == self.pkgrel,
            semver::Op::Greater if epoch < self.epoch => false,
            semver::Op::Greater if epoch > self.epoch => true,
            semver::Op::Greater if self.semver.matches(semver) => true,
            semver::Op::Greater => equal.matches(semver) && pkgrel > self.pkgrel,
            semver::Op::GreaterEq if epoch < self.epoch => false,
            semver::Op::GreaterEq if epoch > self.epoch => true,
            semver::Op::GreaterEq if equal.matches(semver) && pkgrel >= self.pkgrel => true,
            semver::Op::GreaterEq if equal.matches(semver) && pkgrel < self.pkgrel => false,
            semver::Op::GreaterEq => self.semver.matches(semver),
            semver::Op::Less if epoch > self.epoch => false,
            semver::Op::Less if epoch < self.epoch => true,
            semver::Op::Less if equal.matches(semver) && pkgrel < self.pkgrel => true,
            semver::Op::Less => self.semver.matches(semver),
            semver::Op::LessEq if epoch > self.epoch => false,
            semver::Op::LessEq if epoch < self.epoch => true,
            semver::Op::LessEq if equal.matches(semver) && pkgrel <= self.pkgrel => true,
            semver::Op::LessEq if equal.matches(semver) && pkgrel > self.pkgrel => false,
            semver::Op::LessEq => self.semver.matches(semver),
            // Tilde requirements allow the patch part of the semver version (the third number) to increase.
            semver::Op::Tilde if epoch != self.epoch => false,
            semver::Op::Tilde if equal.matches(semver) && pkgrel >= self.pkgrel => true,
            semver::Op::Tilde if equal.matches(semver) && pkgrel < self.pkgrel => false,
            semver::Op::Tilde => self.semver.matches(semver),
            // Caret requirements allow parts that are right of the first nonzero part of the semver version to increase.
            semver::Op::Caret if epoch != self.epoch => false,
            semver::Op::Caret if equal.matches(semver) && pkgrel >= self.pkgrel => true,
            semver::Op::Caret if equal.matches(semver) && pkgrel < self.pkgrel => false,
            semver::Op::Caret => self.semver.matches(semver),
            // bpt does not currently support wildcard due to parsing ambiguities.
            semver::Op::Wildcard => false,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Versions are manually ordered from lowest to highest.  We can leverage the index
    // comparisons to validate the dependency version comparisons.
    const VERSIONS: &[&str] = &[
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

    #[test]
    fn test_common_ops() {
        for (p, provider_ver) in VERSIONS.iter().enumerate() {
            let provider = FieldStr::try_from(*provider_ver)
                .field(PkgVer::NAME)
                .and_then(PkgVer::from_field_str)
                .unwrap();

            for (r, depver) in VERSIONS.iter().enumerate() {
                let depver = *depver;

                for (cmpstr, expect) in &[
                    ("=", p == r),
                    (">", p > r),
                    (">=", p >= r),
                    ("<", p < r),
                    ("<=", p <= r),
                ] {
                    let versionreq = FieldStr::try_from([cmpstr, depver].concat())
                        .and_then(VersionReq::from_field_str)
                        .unwrap();

                    assert!(
                        versionreq.provided_by(&provider) == *expect,
                        "{provider} ({p}) provides {versionreq} ({r}) -> expected {expect}",
                    );
                }
            }
        }
    }

    #[test]
    fn test_tilde() {
        for (provider, passing_depver) in &[
            ("0.9.9", vec!["0.9.9"]),
            ("0.9.9-r1", vec!["0.9.9", "0.9.9-r1"]),
            ("0.9.9-r2", vec!["0.9.9", "0.9.9-r1", "0.9.9-r2"]),
            ("1.0.0", vec!["1.0.0"]),
            ("1.0.0-r1", vec!["1.0.0", "1.0.0-r1"]),
            ("1.0.0-r2", vec!["1.0.0", "1.0.0-r1", "1.0.0-r2"]),
            ("1.0.1", vec!["1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1"]),
            (
                "1.0.1-r1",
                vec!["1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1", "1.0.1-r1"],
            ),
            (
                "1.0.1-r2",
                vec![
                    "1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1", "1.0.1-r1", "1.0.1-r2",
                ],
            ),
            ("e1-0.9.9", vec!["e1-0.9.9"]),
            ("e1-0.9.9-r1", vec!["e1-0.9.9", "e1-0.9.9-r1"]),
            (
                "e1-0.9.9-r2",
                vec!["e1-0.9.9", "e1-0.9.9-r1", "e1-0.9.9-r2"],
            ),
            ("e1-1.0.0", vec!["e1-1.0.0"]),
            ("e1-1.0.0-r1", vec!["e1-1.0.0", "e1-1.0.0-r1"]),
            (
                "e1-1.0.0-r2",
                vec!["e1-1.0.0", "e1-1.0.0-r1", "e1-1.0.0-r2"],
            ),
            (
                "e1-1.0.1",
                vec!["e1-1.0.0", "e1-1.0.0-r1", "e1-1.0.0-r2", "e1-1.0.1"],
            ),
            (
                "e1-1.0.1-r1",
                vec![
                    "e1-1.0.0",
                    "e1-1.0.0-r1",
                    "e1-1.0.0-r2",
                    "e1-1.0.1",
                    "e1-1.0.1-r1",
                ],
            ),
            (
                "e1-1.0.1-r2",
                vec![
                    "e1-1.0.0",
                    "e1-1.0.0-r1",
                    "e1-1.0.0-r2",
                    "e1-1.0.1",
                    "e1-1.0.1-r1",
                    "e1-1.0.1-r2",
                ],
            ),
            ("e2-0.9.9", vec!["e2-0.9.9"]),
            ("e2-0.9.9-r1", vec!["e2-0.9.9", "e2-0.9.9-r1"]),
            (
                "e2-0.9.9-r2",
                vec!["e2-0.9.9", "e2-0.9.9-r1", "e2-0.9.9-r2"],
            ),
            ("e2-1.0.0", vec!["e2-1.0.0"]),
            ("e2-1.0.0-r1", vec!["e2-1.0.0", "e2-1.0.0-r1"]),
            (
                "e2-1.0.0-r2",
                vec!["e2-1.0.0", "e2-1.0.0-r1", "e2-1.0.0-r2"],
            ),
            (
                "e2-1.0.1",
                vec!["e2-1.0.0", "e2-1.0.0-r1", "e2-1.0.0-r2", "e2-1.0.1"],
            ),
            (
                "e2-1.0.1-r1",
                vec![
                    "e2-1.0.0",
                    "e2-1.0.0-r1",
                    "e2-1.0.0-r2",
                    "e2-1.0.1",
                    "e2-1.0.1-r1",
                ],
            ),
            (
                "e2-1.0.1-r2",
                vec![
                    "e2-1.0.0",
                    "e2-1.0.0-r1",
                    "e2-1.0.0-r2",
                    "e2-1.0.1",
                    "e2-1.0.1-r1",
                    "e2-1.0.1-r2",
                ],
            ),
        ] {
            let provider = FieldStr::try_from(*provider)
                .field(PkgVer::NAME)
                .and_then(PkgVer::from_field_str)
                .unwrap();

            for version in VERSIONS {
                let versionreq = FieldStr::try_from(format!("~{version}"))
                    .and_then(VersionReq::from_field_str)
                    .unwrap();

                let expect = passing_depver.contains(version);
                assert!(
                    versionreq.provided_by(&provider) == expect,
                    "{provider} provides {versionreq} -> expected {expect}",
                );
            }
        }
    }

    /// Test tilde vs caret semantic divergence with minor and major version bumps.
    ///
    /// The existing test_tilde and test_caret tests use a version set that only varies in patch
    /// and pkgrel, making their expectations identical.  This test adds minor (1.1.0) and major
    /// (2.0.0) bumps to exercise the actual difference:
    /// - ~1.0.0 (tilde): allows patch increases only (1.0.x), rejects 1.1.0 and 2.0.0
    /// - ^1.0.0 (caret): allows minor+patch increases (1.x.y), rejects 2.0.0
    /// - ^0.1.0 (caret with major=0): allows only patch increases (0.1.x), rejects 0.2.0
    #[test]
    fn test_tilde_vs_caret_divergence() {
        let cases: &[(&str, &str, bool, bool)] = &[
            // (provider,  req_ver,  tilde_expect, caret_expect)
            //
            // Minor bump: tilde rejects, caret accepts (when major >= 1)
            ("1.1.0", "1.0.0", false, true),
            ("1.1.0-r1", "1.0.0", false, true),
            ("1.2.0", "1.0.0", false, true),
            ("1.1.0", "1.0.0-r1", false, true),
            //
            // Major bump: both reject
            ("2.0.0", "1.0.0", false, false),
            ("2.0.0", "1.1.0", false, false),
            ("3.0.0", "1.0.0", false, false),
            //
            // Patch bump: both accept
            ("1.0.1", "1.0.0", true, true),
            ("1.0.2", "1.0.0", true, true),
            //
            // Same version: both accept
            ("1.0.0", "1.0.0", true, true),
            ("1.1.0", "1.1.0", true, true),
            //
            // major=0: caret treats minor as breaking (first nonzero part)
            // ^0.1.0 allows 0.1.x, rejects 0.2.0 — same as ~0.1.0
            ("0.1.1", "0.1.0", true, true),
            ("0.2.0", "0.1.0", false, false),
            //
            // major=0, minor=0: caret treats patch as breaking
            // ^0.0.1 allows only 0.0.1, rejects 0.0.2 — same as ~0.0.1... except
            // tilde allows patch bumps (0.0.x), caret does not
            ("0.0.2", "0.0.1", true, false),
            ("0.0.1", "0.0.1", true, true),
            //
            // Epoch must still match for both
            ("e1-1.1.0", "e1-1.0.0", false, true),
            ("e1-2.0.0", "e1-1.0.0", false, false),
            ("1.1.0", "e1-1.0.0", false, false),
            ("e1-1.0.1", "1.0.0", false, false),
            //
            // Epoch + minor bump with pkgrel
            ("e1-1.1.0-r1", "e1-1.0.0", false, true),
            ("e1-1.1.0", "e1-1.0.0-r1", false, true),
        ];

        for &(provider, req_ver, tilde_expect, caret_expect) in cases {
            let provider = FieldStr::try_from(provider)
                .field(PkgVer::NAME)
                .and_then(PkgVer::from_field_str)
                .unwrap();

            let tilde_req = FieldStr::try_from(format!("~{req_ver}"))
                .and_then(VersionReq::from_field_str)
                .unwrap();
            assert!(
                tilde_req.provided_by(&provider) == tilde_expect,
                "{provider} provides {tilde_req} -> expected {tilde_expect}",
            );

            let caret_req = FieldStr::try_from(format!("^{req_ver}"))
                .and_then(VersionReq::from_field_str)
                .unwrap();
            assert!(
                caret_req.provided_by(&provider) == caret_expect,
                "{provider} provides {caret_req} -> expected {caret_expect}",
            );
        }
    }

    #[test]
    fn test_caret() {
        for (provider, passing_depver) in &[
            ("0.9.9", vec!["0.9.9"]),
            ("0.9.9-r1", vec!["0.9.9", "0.9.9-r1"]),
            ("0.9.9-r2", vec!["0.9.9", "0.9.9-r1", "0.9.9-r2"]),
            ("1.0.0", vec!["1.0.0"]),
            ("1.0.0-r1", vec!["1.0.0", "1.0.0-r1"]),
            ("1.0.0-r2", vec!["1.0.0", "1.0.0-r1", "1.0.0-r2"]),
            ("1.0.1", vec!["1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1"]),
            (
                "1.0.1-r1",
                vec!["1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1", "1.0.1-r1"],
            ),
            (
                "1.0.1-r2",
                vec![
                    "1.0.0", "1.0.0-r1", "1.0.0-r2", "1.0.1", "1.0.1-r1", "1.0.1-r2",
                ],
            ),
            ("e1-0.9.9", vec!["e1-0.9.9"]),
            ("e1-0.9.9-r1", vec!["e1-0.9.9", "e1-0.9.9-r1"]),
            (
                "e1-0.9.9-r2",
                vec!["e1-0.9.9", "e1-0.9.9-r1", "e1-0.9.9-r2"],
            ),
            ("e1-1.0.0", vec!["e1-1.0.0"]),
            ("e1-1.0.0-r1", vec!["e1-1.0.0", "e1-1.0.0-r1"]),
            (
                "e1-1.0.0-r2",
                vec!["e1-1.0.0", "e1-1.0.0-r1", "e1-1.0.0-r2"],
            ),
            (
                "e1-1.0.1",
                vec!["e1-1.0.0", "e1-1.0.0-r1", "e1-1.0.0-r2", "e1-1.0.1"],
            ),
            (
                "e1-1.0.1-r1",
                vec![
                    "e1-1.0.0",
                    "e1-1.0.0-r1",
                    "e1-1.0.0-r2",
                    "e1-1.0.1",
                    "e1-1.0.1-r1",
                ],
            ),
            (
                "e1-1.0.1-r2",
                vec![
                    "e1-1.0.0",
                    "e1-1.0.0-r1",
                    "e1-1.0.0-r2",
                    "e1-1.0.1",
                    "e1-1.0.1-r1",
                    "e1-1.0.1-r2",
                ],
            ),
            ("e2-0.9.9", vec!["e2-0.9.9"]),
            ("e2-0.9.9-r1", vec!["e2-0.9.9", "e2-0.9.9-r1"]),
            (
                "e2-0.9.9-r2",
                vec!["e2-0.9.9", "e2-0.9.9-r1", "e2-0.9.9-r2"],
            ),
            ("e2-1.0.0", vec!["e2-1.0.0"]),
            ("e2-1.0.0-r1", vec!["e2-1.0.0", "e2-1.0.0-r1"]),
            (
                "e2-1.0.0-r2",
                vec!["e2-1.0.0", "e2-1.0.0-r1", "e2-1.0.0-r2"],
            ),
            (
                "e2-1.0.1",
                vec!["e2-1.0.0", "e2-1.0.0-r1", "e2-1.0.0-r2", "e2-1.0.1"],
            ),
            (
                "e2-1.0.1-r1",
                vec![
                    "e2-1.0.0",
                    "e2-1.0.0-r1",
                    "e2-1.0.0-r2",
                    "e2-1.0.1",
                    "e2-1.0.1-r1",
                ],
            ),
            (
                "e2-1.0.1-r2",
                vec![
                    "e2-1.0.0",
                    "e2-1.0.0-r1",
                    "e2-1.0.0-r2",
                    "e2-1.0.1",
                    "e2-1.0.1-r1",
                    "e2-1.0.1-r2",
                ],
            ),
        ] {
            let provider = FieldStr::try_from(*provider)
                .field(PkgVer::NAME)
                .and_then(PkgVer::from_field_str)
                .unwrap();

            for version in VERSIONS {
                let versionreq = FieldStr::try_from(format!("^{}", version))
                    .and_then(VersionReq::from_field_str)
                    .unwrap();

                let expect = passing_depver.contains(version);
                assert!(
                    versionreq.provided_by(&provider) == expect,
                    "{provider} provides {versionreq} -> expected {expect}",
                );
            }
        }
    }
}
