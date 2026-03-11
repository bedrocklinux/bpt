//! A [Bpt] or [Bbuild] file.
use crate::{error::*, file::*, io::QueryCredentials, marshalling::*, metadata::*};
use camino::Utf8Path;
use std::fs::File;

pub enum Pkg {
    Bpt(Bpt),
    Bbuild(Bbuild),
}

impl Pkg {
    pub fn from_file(
        mut file: File,
        pubkeys: &PublicKeys,
        query_credentials: Option<&QueryCredentials>,
        loc: &str,
    ) -> Result<Option<Self>, Err> {
        if file.verify_magic::<Bpt>().is_ok() {
            Bpt::from_file(file, pubkeys)
                .loc(loc)
                .map(Pkg::Bpt)
                .map(Some)
        } else if file.verify_magic::<Bbuild>().is_ok() {
            let query_credentials = query_credentials.ok_or_else(|| {
                Err::InputFieldInvalid(
                    "query credentials",
                    "missing query credentials for bbuild".to_string(),
                )
            })?;
            let query_credentials = query_credentials.get()?;
            Bbuild::from_file(file, pubkeys, query_credentials.as_ref())
                .loc(loc)
                .map(Pkg::Bbuild)
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        match self {
            Pkg::Bpt(bpt) => bpt.link(path),
            Pkg::Bbuild(bbuild) => bbuild.link(path),
        }
    }

    pub fn pkgid(&self) -> &PkgId {
        match self {
            Pkg::Bpt(bpt) => bpt.pkgid(),
            Pkg::Bbuild(bbuild) => bbuild.pkgid(),
        }
    }

    pub fn pkginfo(&self) -> &PkgInfo {
        match self {
            Pkg::Bpt(bpt) => bpt.pkginfo(),
            Pkg::Bbuild(bbuild) => bbuild.pkginfo(),
        }
    }

    pub fn into_file(self) -> File {
        match self {
            Pkg::Bpt(bpt) => bpt.into_file(),
            Pkg::Bbuild(bbuild) => bbuild.into_file(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constant::*, io::FileAux};
    use nix::unistd::Uid;

    fn invalid_root_query_credentials() -> QueryCredentials<'static> {
        let conf = Box::leak(Box::new(
            BptConf::from_file_contents(
                r#"
                [general]
                default-archs = noarch, host
                pin-direct-pkgver = false

                [networking]
                util = curl -f -L {}
                print-stderr = false

                [build]
                unprivileged-user = definitely-missing-bpt-user
                unprivileged-group = definitely-missing-bpt-group
                tmp = /tmp

                [make-repo]
                archs = noarch, host, bbuild

                [cache]
                pkg-max-days = 90
                src-max-days = 90
            "#,
            )
            .unwrap(),
        ));
        QueryCredentials::new_for_euid(conf, Uid::from_raw(0))
    }

    #[test]
    fn bpt_magic_does_not_resolve_query_credentials() {
        let query_credentials = invalid_root_query_credentials();
        let file = File::create_memfd(c"pkg-bpt-test", BPT_MAGIC).unwrap();

        let result = Pkg::from_file(
            file,
            &PublicKeys::from_skipping_verification(),
            Some(&query_credentials),
            "<memfd>",
        );
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("expected invalid synthetic bpt to fail"),
        };

        assert!(
            !matches!(err, Err::InputFieldInvalid("build unprivileged-user", _)),
            "unexpected build credential lookup for bpt branch: {err}"
        );
        assert!(
            !matches!(err, Err::InputFieldInvalid("build unprivileged-group", _)),
            "unexpected build credential lookup for bpt branch: {err}"
        );
    }

    #[test]
    fn bbuild_magic_resolves_query_credentials() {
        let query_credentials = invalid_root_query_credentials();
        let file = File::create_memfd(c"pkg-bbuild-test", BBUILD_MAGIC).unwrap();

        let result = Pkg::from_file(
            file,
            &PublicKeys::from_skipping_verification(),
            Some(&query_credentials),
            "<memfd>",
        );
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("expected invalid synthetic bbuild to fail"),
        };

        assert!(
            matches!(err, Err::InputFieldInvalid("build unprivileged-user", _))
                || matches!(err, Err::InputFieldInvalid("build unprivileged-group", _)),
            "expected lazy build credential lookup error, got {err}"
        );
    }
}
