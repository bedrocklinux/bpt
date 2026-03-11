//! Location a [PkgIdx] or [FileIdx] file.  May be any of:
//!
/// - File path to a [PkgIdx]
/// - File path to a [FileIdx]
/// - http(s) URL to a [PkgIdx]
/// - http(s) URL to a [FileIdx]
use crate::{error::*, file::*, io::*, location::*, make_display_color};
use camino::Utf8Path;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdxPathUrl {
    Path(IdxPath),
    Url(IdxUrl),
}

impl IdxPathUrl {
    pub fn open(
        &self,
        pubkeys: &PublicKeys,
        netutil: &NetUtil,
        dir: Option<&Utf8Path>,
    ) -> Result<Idx, Err> {
        match self {
            IdxPathUrl::Path(p) => p.open(pubkeys, dir),
            IdxPathUrl::Url(u) => u.download(netutil, pubkeys, dir),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            IdxPathUrl::Path(p) => p.as_str(),
            IdxPathUrl::Url(u) => u.as_str(),
        }
    }
}

make_display_color!(IdxPathUrl, |s, f| {
    // The embedded values have colors defined
    match s {
        IdxPathUrl::Path(s) => write!(f, "{}", s.color()),
        IdxPathUrl::Url(s) => write!(f, "{}", s.color()),
    }
});

impl std::fmt::Display for IdxPathUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdxPathUrl::Path(s) => write!(f, "{s}"),
            IdxPathUrl::Url(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for IdxPathUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            IdxUrl::from_str(s).map(IdxPathUrl::Url)
        } else {
            IdxPath::from_str(s).map(IdxPathUrl::Path)
        }
    }
}
