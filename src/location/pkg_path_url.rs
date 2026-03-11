//! Location a [Bpt] or [Bbuild] file.  May be any of:
//!
//! - File path to a [Bpt]
//! - File path to a [Bbuild]
//! - http(s) URL to a [Bpt]
//! - http(s) URL to a [Bbuild]
use crate::{collection::*, error::*, file::*, io::*, location::*};
use camino::Utf8Path;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PkgPathUrl {
    Path(PkgPath),
    Url(PkgUrl),
}

impl PkgPathUrl {
    pub fn open(
        &self,
        netutil: &NetUtil,
        pkgcache: &mut Cache,
        pubkeys: &PublicKeys,
        dir: Option<&Utf8Path>,
        query_credentials: Option<&QueryCredentials>,
    ) -> Result<Pkg, Err> {
        match self {
            PkgPathUrl::Path(p) => p.open(pubkeys, dir, query_credentials),
            PkgPathUrl::Url(u) => u.download(netutil, pkgcache, pubkeys, dir, query_credentials),
        }
    }
}

impl std::fmt::Display for PkgPathUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PkgPathUrl::Path(s) => write!(f, "{s}"),
            PkgPathUrl::Url(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for PkgPathUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            PkgUrl::from_str(s).map(PkgPathUrl::Url)
        } else {
            PkgPath::from_str(s).map(PkgPathUrl::Path)
        }
    }
}
