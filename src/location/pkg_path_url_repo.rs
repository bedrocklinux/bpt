//! Location a [Bpt] or [Bbuild] file.  May be any of:
//!
//! - File path to a [Bpt]
//! - File path to a [Bbuild]
//! - http(s) URL to a [Bpt]
//! - http(s) URL to a [Bbuild]
//! - [PartId] to an repository [Bbuild] file
//! - [PartId] to an repository [Bpt] file
use crate::{error::*, location::*, metadata::*};
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PkgPathUrlRepo {
    Path(PkgPath),
    Url(PkgUrl),
    Repo(PartId),
}

impl std::fmt::Display for PkgPathUrlRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PkgPathUrlRepo::Path(s) => write!(f, "{s}"),
            PkgPathUrlRepo::Url(s) => write!(f, "{s}"),
            PkgPathUrlRepo::Repo(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for PkgPathUrlRepo {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            PkgUrl::from_str(s).map(PkgPathUrlRepo::Url)
        } else if s.contains('/') || s.ends_with(".bpt") || s.ends_with(".bbuild") {
            PkgPath::from_str(s).map(PkgPathUrlRepo::Path)
        } else if let Ok(id) = PartId::from_str(s) {
            Ok(PkgPathUrlRepo::Repo(id))
        } else {
            Err(Err::InvalidPkgPathUrlRepo(s.to_owned()))
        }
    }
}
